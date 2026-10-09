use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::{AGE_7, IntegerProperty, MOISTURE};
use crisp_data::{Block, BlockState, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub trait Crop {
    fn age_property(&self) -> IntegerProperty {
        AGE_7
    }

    fn max_age(&self) -> i32 {
        7
    }

    fn state_for_age(&self, block: Block, age: i32) -> BlockState {
        block.default_state().set(self.age_property(), age)
    }

    fn bonemeal_age_increase(&self, level: &mut ServerLevel) -> i32 {
        mth::next_int(&mut level.random, 2, 5)
    }
}

pub fn age<C: Crop + ?Sized>(crop: &C, state: BlockState) -> i32 {
    state.get(crop.age_property())
}

pub fn is_max_age<C: Crop + ?Sized>(crop: &C, state: BlockState) -> bool {
    age(crop, state) >= crop.max_age()
}

pub fn random_tick<C: Crop + ?Sized>(crop: &C, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if level.raw_brightness(pos, 0) >= 9 {
        let age = age(crop, state);
        if age < crop.max_age() {
            let growth_speed = growth_speed(state.block(), level, pos);
            if level.random.next_int_bounded((25.0f32 / growth_speed) as i32 + 1) == 0 {
                level.set_block(pos, crop.state_for_age(state.block(), age + 1), UPDATE_CLIENTS);
            }
        }
    }
}

pub fn grow_crops<C: Crop + ?Sized>(crop: &C, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    let age = crop.max_age().min(age(crop, state) + crop.bonemeal_age_increase(level));
    level.set_block(pos, crop.state_for_age(state.block(), age), UPDATE_CLIENTS);
}

pub fn growth_speed(block: Block, level: &mut ServerLevel, pos: BlockPos) -> f32 {
    let mut speed = 1.0f32;
    let below = pos.below();
    for xx in -1..=1 {
        for zz in -1..=1 {
            let mut block_speed = 0.0f32;
            let state = level.block_state(below.offset(xx, 0, zz));
            if state.is(&block_tags::GROWS_CROPS) {
                block_speed = 1.0;
                if state.try_get(MOISTURE).unwrap_or(0) > 0 {
                    block_speed = 3.0;
                }
            }
            if xx != 0 || zz != 0 {
                block_speed /= 4.0;
            }
            speed += block_speed;
        }
    }
    let (north, south, west, east) = (pos.north(), pos.south(), pos.west(), pos.east());
    let horizontal = level.block_state(west).is_block(block) || level.block_state(east).is_block(block);
    let vertical = level.block_state(north).is_block(block) || level.block_state(south).is_block(block);
    if horizontal && vertical {
        speed /= 2.0;
    } else {
        let diagonal = level.block_state(west.north()).is_block(block)
            || level.block_state(east.north()).is_block(block)
            || level.block_state(east.south()).is_block(block)
            || level.block_state(west.south()).is_block(block);
        if diagonal {
            speed /= 2.0;
        }
    }
    speed
}

pub fn has_sufficient_light(level: &mut ServerLevel, pos: BlockPos) -> bool {
    level.raw_brightness(pos, 0) >= 8
}

pub fn can_survive<V: Vegetation + ?Sized>(vegetation: &V, level: &mut ServerLevel, pos: BlockPos) -> bool {
    has_sufficient_light(level, pos) && vegetation_block::can_survive(vegetation, level, pos)
}

pub fn may_place_on_crop_support(state: BlockState) -> bool {
    state.is(&block_tags::SUPPORTS_CROPS)
}

pub struct CropBlock;

impl Crop for CropBlock {}

impl Vegetation for CropBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> bool {
        may_place_on_crop_support(state)
    }
}

impl BlockBehaviour for CropBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        can_survive(self, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        random_tick(self, state, level, pos);
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        !is_max_age(self, state)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        grow_crops(self, level, pos, state);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::random::LegacyRandomSource;
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{AGE_7, MOISTURE};
    use crisp_data::{blocks, items};

    use super::growth_speed;
    use crate::server::level::{ServerLevel, testing};
    use crate::world::InteractionHand;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::{self, BlockHitResult};
    use crate::world::level::update_flags::UPDATE_ALL;

    const WHEAT: BlockPos = BlockPos::new(0, -60, 0);

    fn field() -> ServerLevel {
        let mut level = testing::level();
        for x in -1..=1 {
            for z in -1..=1 {
                level.set_block(BlockPos::new(x, -61, z), blocks::FARMLAND.default_state().set(MOISTURE, 7), UPDATE_ALL);
            }
        }
        level.set_block(WHEAT, blocks::WHEAT.default_state(), UPDATE_ALL);
        level
    }

    #[test]
    fn growth_speed_counts_moist_farmland_and_neighbouring_crops() {
        let mut level = field();
        assert_eq!(growth_speed(blocks::WHEAT, &mut level, WHEAT), 10.0);
        level.set_block(WHEAT.east(), blocks::WHEAT.default_state(), UPDATE_ALL);
        assert_eq!(growth_speed(blocks::WHEAT, &mut level, WHEAT), 10.0);
        level.set_block(WHEAT.north(), blocks::WHEAT.default_state(), UPDATE_ALL);
        assert_eq!(growth_speed(blocks::WHEAT, &mut level, WHEAT), 5.0);
    }

    #[test]
    fn crops_grow_on_random_ticks_in_daylight() {
        let mut level = field();
        assert_eq!(level.raw_brightness(WHEAT, 0), 15);
        level.random = LegacyRandomSource::new(42);
        for _ in 0..100 {
            let state = level.block_state(WHEAT);
            block::of(state).random_tick(state, &mut level, WHEAT);
        }
        assert_eq!(level.block_state(WHEAT).get(AGE_7), 7);
    }

    #[test]
    fn crops_do_not_survive_in_the_dark() {
        let mut level = testing::level();
        level.chunks.prepare_ticking_chunk(crisp_core::ChunkPos::new(0, 0));
        level.set_block(WHEAT.below(), blocks::FARMLAND.default_state(), UPDATE_ALL);
        level.set_block(WHEAT, blocks::WHEAT.default_state(), UPDATE_ALL);
        let stone = blocks::STONE.default_state();
        for (dx, dy, dz) in [(0, 1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)] {
            level.set_block(WHEAT.offset(dx, dy, dz), stone, UPDATE_ALL);
        }
        assert_eq!(level.raw_brightness(WHEAT, 0), 15);
        assert!(level.block_state(WHEAT).is_block(blocks::WHEAT));
        level.chunks.run_light_updates();
        assert_eq!(level.raw_brightness(WHEAT, 0), 0);
        level.set_block(WHEAT.east(), blocks::COBBLESTONE.default_state(), UPDATE_ALL);
        assert!(level.block_state(WHEAT).is_air());
    }

    #[test]
    fn bone_meal_ages_a_crop_and_is_used_up() {
        let mut level = field();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::BONE_MEAL, 3));
        let hit = BlockHitResult { location: Vec3::new(0.5, -59.9, 0.5), direction: Direction::Up, block_pos: WHEAT, inside: false, world_border_hit: false };
        assert!(level.use_item_on(player, InteractionHand::MainHand, hit).consumes_action());
        let age = level.block_state(WHEAT).get(AGE_7);
        assert!((2..=5).contains(&age), "age {age}");
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::BONE_MEAL, 2)));
    }
}
