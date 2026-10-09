use crisp_core::BlockPos;
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;

pub struct LoomBlock;

impl BlockBehaviour for LoomBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }

    fn use_without_item(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        InteractionResult::SUCCESS
    }
}
