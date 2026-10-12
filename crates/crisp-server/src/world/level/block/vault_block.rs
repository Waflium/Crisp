use crisp_core::BlockPos;
use crisp_data::properties::{HORIZONTAL_FACING, VAULT_STATE, VaultState};
use crisp_data::{Block, BlockState};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::{InteractionHand, InteractionResult};

pub struct VaultBlock;

impl BlockBehaviour for VaultBlock {
    fn use_item_on(
        &self,
        item_stack: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if item_stack.is_empty() || state.get(VAULT_STATE) != VaultState::Active {
            return InteractionResult::TryWithEmptyHand;
        }
        if !matches!(level.get_block_entity(pos).map(|entity| &entity.kind), Some(BlockEntityKind::Vault(_))) {
            return InteractionResult::TryWithEmptyHand;
        }
        let mut item_in_hand = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        level.vault_try_insert_key(pos, state, player, &mut item_in_hand);
        level.player_mut(player).set_item_in_hand(hand, Some(item_in_hand));
        InteractionResult::SUCCESS_SERVER
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
