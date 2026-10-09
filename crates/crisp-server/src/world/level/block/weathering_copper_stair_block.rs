use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::stair_block::StairBlock;
use super::weathering_copper::change_over_time;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;

pub struct WeatheringCopperStairBlock;

impl BlockBehaviour for WeatheringCopperStairBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        StairBlock.get_state_for_placement(block, ctx, level)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        StairBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_over_time(state, level, pos);
    }
}
