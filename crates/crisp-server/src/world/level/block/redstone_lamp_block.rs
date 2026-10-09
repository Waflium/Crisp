use crisp_core::BlockPos;
use crisp_data::properties::LIT;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct RedstoneLampBlock {
    pub block: Block,
}

impl BlockBehaviour for RedstoneLampBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(LIT, level.has_neighbor_signal(ctx.clicked_pos())))
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let is_lit = state.get(LIT);
        if is_lit != level.has_neighbor_signal(pos) {
            if is_lit {
                level.schedule_tick(pos, self.block, 4);
            } else {
                level.set_block(pos, state.cycle(LIT), UPDATE_CLIENTS);
            }
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(LIT) && !level.has_neighbor_signal(pos) {
            level.set_block(pos, state.cycle(LIT), UPDATE_CLIENTS);
        }
    }
}
