use crisp_data::properties::{HORIZONTAL_FACING, IntegerProperty};
use crisp_data::{Block, BlockState};

use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub fn can_be_replaced(state: BlockState, ctx: &BlockPlaceContext, segment: IntegerProperty) -> bool {
    !ctx.is_secondary_use_active() && !ctx.item.is_empty() && ctx.item.item == state.block().item() && state.get(segment) < 4
}

pub fn get_state_for_placement(ctx: &BlockPlaceContext, level: &mut ServerLevel, block: Block, segment: IntegerProperty) -> BlockState {
    let state = level.block_state(ctx.clicked_pos());
    if state.is_block(block) {
        state.set(segment, 4.min(state.get(segment) + 1))
    } else {
        block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite())
    }
}
