use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::horizontal_directional_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;

pub struct CarvedPumpkinBlock;

impl BlockBehaviour for CarvedPumpkinBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        horizontal_directional_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        horizontal_directional_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }
}
