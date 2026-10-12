use crisp_core::BlockPos;
use crisp_data::properties::POWERED;
use crisp_data::{Block, BlockState};

use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> BlockState {
    block.default_state().set(POWERED, level.has_neighbor_signal(ctx.clicked_pos()))
}

pub fn neighbor_changed(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let signal = level.has_neighbor_signal(pos);
    if signal != state.get(POWERED) {
        level.set_block(pos, state.set(POWERED, signal), UPDATE_CLIENTS);
    }
}
