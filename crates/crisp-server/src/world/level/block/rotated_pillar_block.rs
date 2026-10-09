use crisp_data::properties::AXIS;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;

pub struct RotatedPillarBlock;

impl BlockBehaviour for RotatedPillarBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(AXIS, ctx.clicked_face().axis()))
    }
}
