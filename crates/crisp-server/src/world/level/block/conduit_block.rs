use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState, fluid_tags};

use super::BlockBehaviour;
use super::simple_waterlogged_block::schedule_water_tick;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct ConduitBlock;

impl BlockBehaviour for ConduitBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let fluid_state = level.fluid_state(ctx.clicked_pos());
        Some(block.default_state().set(WATERLOGGED, fluid_state.is(&fluid_tags::WATER) && fluid_state.amount == 8))
    }
}
