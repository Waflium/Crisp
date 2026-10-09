use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;

pub struct SmithingTableBlock;

impl BlockBehaviour for SmithingTableBlock {
    fn use_without_item(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        InteractionResult::SUCCESS
    }
}
