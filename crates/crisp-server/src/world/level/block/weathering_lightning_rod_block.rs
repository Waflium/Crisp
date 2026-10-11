use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::lightning_rod_block::LightningRodBlock;
use super::weathering_copper::change_over_time;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct WeatheringLightningRodBlock;

impl BlockBehaviour for WeatheringLightningRodBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        LightningRodBlock.get_state_for_placement(block, ctx, level)
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
        LightningRodBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn own_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> i32 {
        LightningRodBlock.own_signal(state, level, pos)
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        LightningRodBlock.get_direct_signal(state, level, pos, direction)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        LightningRodBlock.tick(state, level, pos);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        LightningRodBlock.affect_neighbors_after_removal(state, level, pos, moved_by_piston);
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {
        LightningRodBlock.on_place(state, level, pos, old_state, moved_by_piston);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_over_time(state, level, pos);
    }
}
