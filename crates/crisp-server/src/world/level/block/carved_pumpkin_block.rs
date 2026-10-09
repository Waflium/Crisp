use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;

pub struct CarvedPumpkinBlock;

impl BlockBehaviour for CarvedPumpkinBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }
}
