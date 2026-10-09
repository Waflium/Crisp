use crisp_core::{BlockPos, Direction};
use crisp_data::properties::SEGMENT_AMOUNT;
use crisp_data::{Block, BlockState, SupportType};

use super::BlockBehaviour;
use super::segmentable_block;
use super::vegetation_block;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub struct LeafLitterBlock;

impl BlockBehaviour for LeafLitterBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(segmentable_block::get_state_for_placement(ctx, level, block, SEGMENT_AMOUNT))
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        segmentable_block::can_be_replaced(state, ctx, SEGMENT_AMOUNT) || vegetation_block::block_can_be_replaced(state, ctx)
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        level.block_state(pos.below()).is_face_sturdy(Direction::Up, SupportType::Full)
    }
}
