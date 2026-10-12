use crisp_core::BlockPos;
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState};

use super::horizontal_directional_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;

pub struct LoomBlock;

impl BlockBehaviour for LoomBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        horizontal_directional_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        horizontal_directional_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        level.open_menu(player, crate::world::inventory::menu_provider::MenuProvider::Loom(pos));
        level.award_stat(player, crate::stats::stats::INTERACT_WITH_LOOM, 1);
        InteractionResult::SUCCESS
    }
}
