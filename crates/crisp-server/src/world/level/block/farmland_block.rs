use crisp_core::{BlockPos, Direction};
use crisp_data::properties::MOISTURE;
use crisp_core::random::RandomSource;
use crisp_data::{Block, BlockState, block_tags, fluid_tags, game_rule};

use super::BlockBehaviour;
use super::vegetation_block::{block_field, fields};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS};

pub struct FarmlandBlock {
    base_block: Block,
}

impl FarmlandBlock {
    pub fn new(block: Block) -> FarmlandBlock {
        FarmlandBlock { base_block: block_field(&fields(block), "baseBlock") }
    }

    fn turn_to_base_block(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let new_state = super::push_entities_up(state, self.base_block.default_state(), level, pos);
        level.set_block(pos, new_state, UPDATE_ALL);
    }
}

fn should_maintain_farmland(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    level.block_state(pos.above()).is(&block_tags::MAINTAINS_FARMLAND)
}

fn is_near_water(level: &mut ServerLevel, pos: BlockPos) -> bool {
    for x in pos.x - 4..=pos.x + 4 {
        for y in pos.y..=pos.y + 1 {
            for z in pos.z - 4..=pos.z + 4 {
                if level.fluid_state(BlockPos::new(x, y, z)).is(&fluid_tags::WATER) {
                    return true;
                }
            }
        }
    }
    false
}

impl BlockBehaviour for FarmlandBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let default = block.default_state();
        Some(if !self.can_survive(default, level, ctx.clicked_pos()) { self.base_block.default_state() } else { default })
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Up && !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, state.block(), 1);
        }
        state
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let above_state = level.block_state(pos.above());
        !above_state.is_solid() || should_maintain_farmland(level, pos)
    }

    fn fall_on(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos, entity: EntityId, fall_distance: f64) {
        if ((level.random.next_float() as f64) < fall_distance - 0.5) && level.is_living(entity) {
            let can_trample = if level.is_player(entity) {
                !level.is_under_spawn_protection(pos, entity)
            } else {
                level.game_rules.get_bool(game_rule::MOB_GRIEFING)
            };
            let dimensions = level.entity(entity).dimensions;
            if can_trample && dimensions.width * dimensions.width * dimensions.height > 0.512 {
                self.turn_to_base_block(state, level, pos);
            }
        }
        super::block_fall_on(level, state, pos, entity, fall_distance);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.can_survive(state, level, pos) {
            self.turn_to_base_block(state, level, pos);
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let moisture = state.get(MOISTURE);
        if !is_near_water(level, pos) {
            if moisture > 0 {
                level.set_block(pos, state.set(MOISTURE, moisture - 1), UPDATE_CLIENTS);
            } else if !should_maintain_farmland(level, pos) {
                self.turn_to_base_block(state, level, pos);
            }
        } else if moisture < 7 {
            level.set_block(pos, state.set(MOISTURE, 7), UPDATE_CLIENTS);
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::MOISTURE;

    use crate::server::level::{ServerLevel, testing};
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    const FARMLAND: BlockPos = BlockPos::new(0, -61, 0);

    fn random_tick(level: &mut ServerLevel) {
        let state = level.block_state(FARMLAND);
        block::of(state).random_tick(state, level, FARMLAND);
    }

    #[test]
    fn farmland_dries_out_away_from_water_and_turns_to_dirt() {
        let mut level = testing::level();
        level.set_block(FARMLAND, blocks::FARMLAND.default_state().set(MOISTURE, 1), UPDATE_ALL);
        random_tick(&mut level);
        assert_eq!(level.block_state(FARMLAND).get(MOISTURE), 0);
        random_tick(&mut level);
        assert_eq!(level.block_state(FARMLAND), blocks::DIRT.default_state());
    }

    #[test]
    fn crops_keep_dry_farmland_and_water_moistens_it() {
        let mut level = testing::level();
        level.set_block(FARMLAND, blocks::FARMLAND.default_state(), UPDATE_ALL);
        level.set_block(FARMLAND.above(), blocks::WHEAT.default_state(), UPDATE_ALL);
        random_tick(&mut level);
        assert_eq!(level.block_state(FARMLAND), blocks::FARMLAND.default_state());
        level.set_block(FARMLAND.offset(4, 0, -4), blocks::WATER.default_state(), UPDATE_ALL);
        random_tick(&mut level);
        assert_eq!(level.block_state(FARMLAND).get(MOISTURE), 7);
    }

    #[test]
    fn a_solid_block_on_farmland_turns_it_to_dirt_a_tick_later() {
        let mut level = testing::level();
        level.set_block(FARMLAND, blocks::FARMLAND.default_state(), UPDATE_ALL);
        level.set_block(FARMLAND.above(), blocks::STONE.default_state(), UPDATE_ALL);
        assert!(level.has_scheduled_tick(FARMLAND, blocks::FARMLAND));
        let state = level.block_state(FARMLAND);
        block::of(state).tick(state, &mut level, FARMLAND);
        assert_eq!(level.block_state(FARMLAND), blocks::DIRT.default_state());
    }
}
