use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{AGE_15, BooleanProperty, EAST, NORTH, SOUTH, UP, WATERLOGGED, WEST};
use crisp_data::{Block, BlockState, FIRE_ODDS, blocks};

use super::{BlockBehaviour, base_fire_block};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_NONE};

const MAX_AGE: i32 = 15;
const FIRE_DAMAGE: f32 = 1.0;

fn property_by_direction(direction: Direction) -> Option<BooleanProperty> {
    match direction {
        Direction::North => Some(NORTH),
        Direction::East => Some(EAST),
        Direction::South => Some(SOUTH),
        Direction::West => Some(WEST),
        Direction::Up => Some(UP),
        Direction::Down => None,
    }
}

fn ignite_odds(state: BlockState) -> i32 {
    if state.try_get(WATERLOGGED) == Some(true) { 0 } else { FIRE_ODDS[state.block().0 as usize].0 as i32 }
}

fn burn_odds(state: BlockState) -> i32 {
    if state.try_get(WATERLOGGED) == Some(true) { 0 } else { FIRE_ODDS[state.block().0 as usize].1 as i32 }
}

pub fn can_burn(state: BlockState) -> bool {
    ignite_odds(state) > 0
}

pub fn get_state_for_placement(level: &mut dyn LevelReader, pos: BlockPos) -> BlockState {
    let below_state = level.block_state(pos.below());
    let mut result = blocks::FIRE.default_state();
    if !can_burn(below_state) && !below_state.is_face_sturdy_full(Direction::Up) {
        for direction in Direction::VALUES {
            if let Some(property) = property_by_direction(direction) {
                result = result.set(property, can_burn(level.block_state(pos.relative(direction))));
            }
        }
    }
    result
}

fn fire_tick_delay(level: &mut ServerLevel) -> i32 {
    30 + level.random.next_int_bounded(10)
}

fn is_valid_fire_location(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    Direction::VALUES.into_iter().any(|direction| can_burn(level.block_state(pos.relative(direction))))
}

fn ignite_odds_at(level: &mut ServerLevel, pos: BlockPos) -> i32 {
    if !level.block_state(pos).is_air() {
        return 0;
    }
    Direction::VALUES.into_iter().map(|direction| ignite_odds(level.block_state(pos.relative(direction)))).fold(0, i32::max)
}

fn state_with_age(level: &mut dyn LevelReader, pos: BlockPos, age: i32) -> BlockState {
    let state = base_fire_block::get_state(level, pos);
    if state.is_block(blocks::FIRE) { state.set(AGE_15, age) } else { state }
}

fn is_near_rain(level: &mut ServerLevel, pos: BlockPos) -> bool {
    [pos, pos.west(), pos.east(), pos.north(), pos.south()].into_iter().any(|p| level.is_raining_at(p))
}

fn check_burn_out(level: &mut ServerLevel, pos: BlockPos, chance: i32, age: i32) {
    let odds = burn_odds(level.block_state(pos));
    if level.random.next_int_bounded(chance) < odds {
        let old_state = level.block_state(pos);
        if level.random.next_int_bounded(age + 10) < 5 && !level.is_raining_at(pos) {
            let new_age = (age + level.random.next_int_bounded(5) / 4).min(MAX_AGE);
            let state = state_with_age(level, pos, new_age);
            level.set_block(pos, state, UPDATE_ALL);
        } else {
            level.remove_block(pos, false);
        }
        if old_state.block().is_instance_of("TntBlock") {
            super::tnt_block::prime(level, pos, None, None);
        }
    }
}

pub struct FireBlock;

impl BlockBehaviour for FireBlock {
    fn get_state_for_placement(&self, _block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(get_state_for_placement(level, ctx.clicked_pos()))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if self.can_survive(state, level, pos) { state_with_age(level, pos, state.get(AGE_15)) } else { blocks::AIR.default_state() }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.below()).is_face_sturdy_full(Direction::Up) || is_valid_fire_location(level, pos)
    }

    fn tick(&self, mut state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let delay = fire_tick_delay(level);
        level.schedule_tick(pos, blocks::FIRE, delay);
        if !level.can_spread_fire_around(pos) {
            return;
        }
        if !self.can_survive(state, level, pos) {
            level.remove_block(pos, false);
        }
        let below_state = level.block_state(pos.below());
        let infini_burn = below_state.is(level.chunks.dimension.infiniburn);
        let age = state.get(AGE_15);
        if !infini_burn && level.is_raining() && is_near_rain(level, pos) && level.random.next_float() < 0.2 + age as f32 * 0.03 {
            level.remove_block(pos, false);
            return;
        }
        let new_age = (age + level.random.next_int_bounded(3) / 2).min(MAX_AGE);
        if age != new_age {
            state = state.set(AGE_15, new_age);
            level.set_block(pos, state, UPDATE_NONE);
        }
        if !infini_burn {
            if !is_valid_fire_location(level, pos) {
                let below = pos.below();
                if !level.block_state(below).is_face_sturdy_full(Direction::Up) || age > 3 {
                    level.remove_block(pos, false);
                }
                return;
            }
            if age == MAX_AGE && level.random.next_int_bounded(4) == 0 && !can_burn(level.block_state(pos.below())) {
                level.remove_block(pos, false);
                return;
            }
        }
        let increased_burnout = level.increased_fire_burnout(pos);
        let extra = if increased_burnout { -50 } else { 0 };
        check_burn_out(level, pos.east(), 300 + extra, age);
        check_burn_out(level, pos.west(), 300 + extra, age);
        check_burn_out(level, pos.below(), 250 + extra, age);
        check_burn_out(level, pos.above(), 250 + extra, age);
        check_burn_out(level, pos.north(), 300 + extra, age);
        check_burn_out(level, pos.south(), 300 + extra, age);
        for xx in -1..=1 {
            for zz in -1..=1 {
                for yy in -1..=4 {
                    if xx == 0 && yy == 0 && zz == 0 {
                        continue;
                    }
                    let rate = if yy > 1 { 100 + (yy - 1) * 100 } else { 100 };
                    let test_pos = pos.offset(xx, yy, zz);
                    let ignite_odds = ignite_odds_at(level, test_pos);
                    if ignite_odds <= 0 {
                        continue;
                    }
                    let mut odds = (ignite_odds + 40 + level.difficulty.id() * 7) / (age + 30);
                    if increased_burnout {
                        odds /= 2;
                    }
                    if odds > 0 && level.random.next_int_bounded(rate) <= odds && (!level.is_raining() || !is_near_rain(level, test_pos)) {
                        let spread_age = (age + level.random.next_int_bounded(5) / 4).min(MAX_AGE);
                        let state = state_with_age(level, test_pos, spread_age);
                        level.set_block(test_pos, state, UPDATE_ALL);
                    }
                }
            }
        }
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        base_fire_block::on_place(state, level, pos, old_state);
        let delay = fire_tick_delay(level);
        level.schedule_tick(pos, blocks::FIRE, delay);
    }

    fn entity_inside(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _entity: EntityId, effects: &mut InsideBlockEffects, _is_precise: bool) {
        base_fire_block::entity_inside(effects, FIRE_DAMAGE);
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _player: EntityId) -> BlockState {
        base_fire_block::player_will_destroy(level, pos, state)
    }
}

impl ServerLevel {
    pub fn increased_fire_burnout(&mut self, pos: BlockPos) -> bool {
        self.environment_attributes().get_value_at(crate::world::attribute::environment_attributes::INCREASED_FIRE_BURNOUT, pos)
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{AGE_15, EAST};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn flint_and_steel_lights_fire_and_wears() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::FLINT_AND_STEEL, 1));
        let grass = BlockPos::new(0, -61, 0);
        block::click(&mut level, player, grass, Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        assert_eq!(level.block_state(grass.above()), blocks::FIRE.default_state());
        assert_eq!(level.player(player).inventory.slots[36].as_ref().unwrap().damage_value(), 1);
    }

    #[test]
    fn fire_clings_to_what_burns_and_soul_fire_to_soul_blocks() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 0.0, 0.0);
        let air = BlockPos::new(0, -55, 0);
        level.set_block(air.east(), blocks::OAK_PLANKS.default_state(), UPDATE_ALL);
        assert_eq!(super::base_fire_block::get_state(&mut level, air), blocks::FIRE.default_state().set(EAST, true));
        let soul = BlockPos::new(2, -61, 2);
        level.set_block(soul, blocks::SOUL_SAND.default_state(), UPDATE_ALL);
        assert_eq!(super::base_fire_block::get_state(&mut level, soul.above()), blocks::SOUL_FIRE.default_state());
    }

    #[test]
    fn fire_without_fuel_burns_out_and_planks_burn_away() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 0.0, 0.0);
        let fire = BlockPos::new(0, -60, 0);
        level.set_block(fire, blocks::FIRE.default_state(), UPDATE_ALL);
        for _ in 0..200 {
            let state = level.block_state(fire);
            if !state.is_block(blocks::FIRE) {
                break;
            }
            block::of(state).tick(state, &mut level, fire);
        }
        assert!(level.block_state(fire).is_air());
        let planks = BlockPos::new(4, -60, 4);
        level.set_block(planks, blocks::OAK_PLANKS.default_state(), UPDATE_ALL);
        let mut burnt = false;
        for _ in 0..5000 {
            let state = blocks::FIRE.default_state().set(AGE_15, 10);
            level.set_block(planks.above(), state, UPDATE_ALL);
            block::of(state).tick(state, &mut level, planks.above());
            if !level.block_state(planks).is_block(blocks::OAK_PLANKS) {
                burnt = true;
                break;
            }
        }
        assert!(burnt);
    }

    #[test]
    fn items_burn_in_fire_unless_resistant() {
        use crate::world::entity::LevelEntity;
        use crate::world::entity::item::ItemEntity;
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
        let fire = BlockPos::new(0, -60, 0);
        level.set_block(fire, blocks::FIRE.default_state(), UPDATE_ALL);
        let mut stick = ItemEntity::with_movement(Vec3::new(0.5, -60.0, 0.5), ItemStack::new(items::STICK, 1), Vec3::ZERO);
        stick.pickup_delay = 100;
        let mut netherite = ItemEntity::with_movement(Vec3::new(0.5, -60.0, 0.5), ItemStack::new(items::NETHERITE_INGOT, 1), Vec3::ZERO);
        netherite.pickup_delay = 100;
        let (stick_id, netherite_id) = (stick.entity.id, netherite.entity.id);
        level.add_fresh_entity(LevelEntity::Item(stick));
        level.add_fresh_entity(LevelEntity::Item(netherite));
        for _ in 0..40 {
            level.tick();
        }
        assert!(!level.entities.contains_key(&stick_id));
        assert!(level.entities.contains_key(&netherite_id));
    }
}
