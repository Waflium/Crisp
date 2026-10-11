use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::lantern_block::LanternBlock;
use super::weathering_copper::change_over_time;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct WeatheringLanternBlock;

impl BlockBehaviour for WeatheringLanternBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        LanternBlock.get_state_for_placement(block, ctx, level)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        LanternBlock.can_survive(state, level, pos)
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
        LanternBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_over_time(state, level, pos);
    }
}
