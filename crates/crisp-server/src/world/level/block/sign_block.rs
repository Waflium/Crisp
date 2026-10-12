use crisp_core::{BlockPos, Vec3};
use crisp_data::BlockState;
use crisp_data::packets::play::clientbound;

use super::ceiling_hanging_sign_block::CeilingHangingSignBlock;
use super::standing_sign_block::StandingSignBlock;
use super::wall_hanging_sign_block::WallHangingSignBlock;
use super::wall_sign_block::WallSignBlock;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::sign_applicator::SignApplicator;
use crate::world::level::block::entity::sign_block_entity::SignTextSlot;
use crate::world::{InteractionHand, InteractionResult};

pub trait SignBlock {
    fn get_y_rotation_degrees(&self, state: BlockState) -> f32;

    fn get_sign_hitbox_center_position(&self, _state: BlockState) -> Vec3 {
        Vec3::new(0.5, 0.5, 0.5)
    }
}

pub fn sign_block(state: BlockState) -> Option<&'static dyn SignBlock> {
    let block = state.block();
    if block.is_instance_of("StandingSignBlock") {
        Some(&StandingSignBlock)
    } else if block.is_instance_of("WallSignBlock") {
        Some(&WallSignBlock)
    } else if block.is_instance_of("CeilingHangingSignBlock") {
        Some(&CeilingHangingSignBlock)
    } else if block.is_instance_of("WallHangingSignBlock") {
        Some(&WallHangingSignBlock)
    } else {
        None
    }
}

fn other_player_is_editing_sign(level: &mut ServerLevel, player: EntityId, pos: BlockPos) -> bool {
    let player_uuid = level.player(player).profile.uuid;
    level.sign(pos).and_then(|sign| sign.get_player_who_may_edit()).is_some_and(|player_who_may_edit| player_who_may_edit != player_uuid)
}

pub fn use_item_on(item_stack: ItemStack, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand) -> InteractionResult {
    let Some(sign) = level.sign(pos) else { return InteractionResult::Pass };
    let is_waxed = sign.is_waxed();
    let sign_applicator = SignApplicator::of(item_stack.item);
    let Some(sign_applicator) = sign_applicator.filter(|_| level.may_build(player)) else { return InteractionResult::TryWithEmptyHand };
    if is_waxed || other_player_is_editing_sign(level, player, pos) {
        return InteractionResult::TryWithEmptyHand;
    }
    let text_slot = level.sign_get_slot_player_is_facing(pos, player);
    let text_filtering_enabled = level.player(player).client.text_filtering;
    let can_apply = level.sign(pos).is_some_and(|sign| sign_applicator.can_apply_to_sign(sign.get_text(text_slot), text_filtering_enabled));
    if can_apply && sign_applicator.try_apply_to_sign(level, pos, text_slot, &item_stack) {
        level.sign_execute_click_commands_if_present(player, pos, text_slot);
        if !level.has_infinite_materials(player) {
            let mut item_stack = item_stack;
            item_stack.shrink(1);
            level.player_mut(player).set_item_in_hand(hand, Some(item_stack));
        }
        InteractionResult::SUCCESS
    } else {
        InteractionResult::TryWithEmptyHand
    }
}

pub fn use_without_item(level: &mut ServerLevel, pos: BlockPos, player: EntityId) -> InteractionResult {
    if level.sign(pos).is_none() {
        return InteractionResult::Pass;
    }
    let text_slot = level.sign_get_slot_player_is_facing(pos, player);
    let executed_click_command = level.sign_execute_click_commands_if_present(player, pos, text_slot);
    let text_filtering_enabled = level.player(player).client.text_filtering;
    let Some(sign) = level.sign(pos) else { return InteractionResult::Pass };
    let is_waxed = sign.is_waxed();
    let has_editable_text = sign.get_text(text_slot).has_editable_text(text_filtering_enabled);
    if is_waxed {
        let sound = crisp_data::registry_id(crisp_data::registry::SOUND_EVENT, level.sign_get_sign_interaction_failed_sound_event(pos));
        level.play_sound(None, pos, sound, SoundSource::Blocks, 1.0, 1.0);
        InteractionResult::SUCCESS_SERVER
    } else if executed_click_command {
        InteractionResult::SUCCESS_SERVER
    } else if !other_player_is_editing_sign(level, player, pos) && level.may_build(player) && has_editable_text {
        open_text_edit(level, player, pos, text_slot);
        InteractionResult::SUCCESS_SERVER
    } else {
        InteractionResult::Pass
    }
}

pub fn open_text_edit(level: &mut ServerLevel, player: EntityId, pos: BlockPos, slot: SignTextSlot) {
    let player_uuid = level.player(player).profile.uuid;
    if let Some(sign) = level.sign(pos) {
        sign.set_allowed_player_editor(Some(player_uuid));
    }
    let state = crate::world::level::BlockGetter::block_state(level, pos);
    let p = level.player(player);
    p.connection.send(clientbound::BLOCK_UPDATE, |w| {
        w.position(pos.x, pos.y, pos.z);
        w.varint(state.id() as i32);
    });
    p.connection.send(clientbound::OPEN_SIGN_EDITOR, |w| {
        w.position(pos.x, pos.y, pos.z);
        w.varint(slot.id());
    });
}

pub fn set_placed_by(level: &mut ServerLevel, pos: BlockPos, placer: Option<EntityId>) {
    let Some(player) = placer.filter(|&placer| level.is_player(placer)) else { return };
    let text_filtering_enabled = level.player(player).client.text_filtering;
    let Some(sign) = level.sign(pos) else { return };
    if !sign.is_waxed() && sign.get_text(SignTextSlot::Front).has_editable_text(text_filtering_enabled) {
        open_text_edit(level, player, pos, SignTextSlot::Front);
    }
}
