use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{DOWN, EAST, NORTH, SOUTH, UP, WEST};
use crisp_data::{Block, BlockState, block_tags, blocks};

use super::BlockBehaviour;
use super::pipe_block::property_by_direction;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_LIMIT;
use crate::world::level::{LevelAccessor, LevelReader};

pub fn get_state_with_connections(level: &mut dyn LevelReader, pos: BlockPos, default_state: BlockState) -> BlockState {
    let down = level.block_state(pos.below());
    let up = level.block_state(pos.above());
    let north = level.block_state(pos.north());
    let east = level.block_state(pos.east());
    let south = level.block_state(pos.south());
    let west = level.block_state(pos.west());
    let block = default_state.block();
    let connects = |state: BlockState| state.is_block(block) || state.is_block(blocks::CHORUS_FLOWER);
    default_state
        .try_set(DOWN, connects(down) || down.is(&block_tags::SUPPORTS_CHORUS_PLANT))
        .try_set(UP, connects(up))
        .try_set(NORTH, connects(north))
        .try_set(EAST, connects(east))
        .try_set(SOUTH, connects(south))
        .try_set(WEST, connects(west))
}

pub struct ChorusPlantBlock {
    block: Block,
}

impl ChorusPlantBlock {
    pub fn new(block: Block) -> ChorusPlantBlock {
        ChorusPlantBlock { block }
    }
}

impl BlockBehaviour for ChorusPlantBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(get_state_with_connections(level, ctx.clicked_pos(), block.default_state()))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        if !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, self.block, 1);
            return state;
        }
        let connect = neighbour_state.is_block(self.block)
            || neighbour_state.is_block(blocks::CHORUS_FLOWER)
            || (direction == Direction::Down && neighbour_state.is(&block_tags::SUPPORTS_CHORUS_PLANT));
        state.set(property_by_direction(direction), connect)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.can_survive(state, level, pos) {
            level.destroy_block(pos, true, None, UPDATE_LIMIT);
        }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let below_state = level.block_state(pos.below());
        let block_above_or_below = !level.block_state(pos.above()).is_air() && !below_state.is_air();
        for direction in Direction::PLANE_HORIZONTAL {
            let neighbor_pos = pos.relative(direction);
            if level.block_state(neighbor_pos).is_block(self.block) {
                if block_above_or_below {
                    return false;
                }
                let below = level.block_state(neighbor_pos.below());
                if below.is_block(self.block) || below.is(&block_tags::SUPPORTS_CHORUS_PLANT) {
                    return true;
                }
            }
        }
        below_state.is_block(self.block) || below_state.is(&block_tags::SUPPORTS_CHORUS_PLANT)
    }
}
