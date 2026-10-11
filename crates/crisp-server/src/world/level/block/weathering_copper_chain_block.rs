use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::chain_block::ChainBlock;
use super::weathering_copper::change_over_time;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::LevelAccessor;

pub struct WeatheringCopperChainBlock;

impl BlockBehaviour for WeatheringCopperChainBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        ChainBlock.get_state_for_placement(block, ctx, level)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        ChainBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_over_time(state, level, pos);
    }
}
