use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::LevelAccessor;

pub struct MangroveRootsBlock;

impl BlockBehaviour for MangroveRootsBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(WATERLOGGED, is_water_source(level, ctx.clicked_pos())))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }
}
