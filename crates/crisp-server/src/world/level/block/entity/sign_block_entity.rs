use crisp_core::{BlockPos, mth};
use crisp_data::block_entity_types as types;
use crisp_protocol::nbt::Nbt;

use super::BlockEntityKind;
use super::sign_text::SignText;
use crate::network::chat::{ChatFormatting, ClickEvent, Component};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::level::block::sign_block;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignTextSlot {
    Back,
    Front,
}

impl SignTextSlot {
    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn by_id(id: i32) -> SignTextSlot {
        if id == 1 { SignTextSlot::Front } else { SignTextSlot::Back }
    }
}

#[derive(Clone)]
pub struct FilteredText {
    pub raw: String,
    pub filtered: Option<String>,
}

impl FilteredText {
    pub fn pass_through(raw: String) -> FilteredText {
        FilteredText { filtered: Some(raw.clone()), raw }
    }

    pub fn filtered_or_empty(&self) -> &str {
        self.filtered.as_deref().unwrap_or("")
    }
}

#[derive(Clone, Default)]
pub struct SignBlockEntity {
    player_who_may_edit: Option<u128>,
    front_text: SignText,
    back_text: SignText,
    is_waxed: bool,
    allow_op_features: bool,
}

fn update_messages(new_lines: &[FilteredText], original_text: &SignText, should_filter: bool) -> SignText {
    let mut result = original_text.as_mutable();
    let original_lines = original_text.get_messages(should_filter);
    for (i, line) in new_lines.iter().enumerate() {
        let current_text_style = &original_lines[i].style;
        if should_filter {
            result.set_line(i, Component::literal(line.filtered_or_empty()).with_style(current_text_style));
        } else {
            result.set_line_filtered(
                i,
                Component::literal(line.raw.clone()).with_style(current_text_style),
                Component::literal(line.filtered_or_empty()).with_style(current_text_style),
            );
        }
    }
    result.as_immutable()
}

impl SignBlockEntity {
    pub fn get_text(&self, slot: SignTextSlot) -> &SignText {
        match slot {
            SignTextSlot::Front => &self.front_text,
            SignTextSlot::Back => &self.back_text,
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if self.allow_op_features {
            output.push(("allow_op_features".into(), Nbt::Byte(1)));
        }
        output.push(("front_text".into(), self.front_text.to_nbt()));
        output.push(("back_text".into(), self.back_text.to_nbt()));
        output.push(("is_waxed".into(), Nbt::Byte(self.is_waxed as i8)));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        let flag = |key: &str| input.get(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        self.allow_op_features = flag("allow_op_features");
        let text = |key: &str| input.get(key).and_then(|text| dc::SIGN_TEXT_FRONT.decode(text).ok()).and_then(|text| SignText::from_nbt(&text)).unwrap_or_default();
        self.front_text = text("front_text");
        self.back_text = text("back_text");
        self.is_waxed = flag("is_waxed");
    }

    pub fn can_execute_click_commands(&self, slot: SignTextSlot, text_filtering_enabled: bool) -> bool {
        self.is_waxed && self.get_text(slot).has_any_click_commands(text_filtering_enabled)
    }

    pub fn set_allowed_player_editor(&mut self, player_uuid: Option<u128>) {
        self.player_who_may_edit = player_uuid;
    }

    pub fn get_player_who_may_edit(&self) -> Option<u128> {
        self.player_who_may_edit
    }

    pub fn is_waxed(&self) -> bool {
        self.is_waxed
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.front_text = get(dc::SIGN_TEXT_FRONT).and_then(SignText::from_nbt).unwrap_or_default();
        self.back_text = get(dc::SIGN_TEXT_BACK).and_then(SignText::from_nbt).unwrap_or_default();
        self.is_waxed = get(dc::WAXED).is_some();
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        let mut set = |ty: DataComponentType, value: Option<Nbt>| {
            components.retain(|(t, _)| *t != ty);
            components.push((ty, value));
        };
        set(dc::SIGN_TEXT_FRONT, Some(self.front_text.to_nbt()));
        set(dc::SIGN_TEXT_BACK, Some(self.back_text.to_nbt()));
        if self.is_waxed {
            set(dc::WAXED, Some(Nbt::Compound(Vec::new())));
        }
    }
}

impl ServerLevel {
    pub fn sign(&mut self, pos: BlockPos) -> Option<&mut SignBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Sign(sign) => Some(sign),
            _ => None,
        }
    }

    pub fn sign_get_slot_player_is_facing(&mut self, pos: BlockPos, player: EntityId) -> SignTextSlot {
        let Some(state) = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) else { return SignTextSlot::Front };
        let Some(sign) = sign_block::sign_block(state) else { return SignTextSlot::Front };
        let sign_position_offset = sign.get_sign_hitbox_center_position(state);
        let player_pos = self.entity(player).position();
        let xd = player_pos.x - (pos.x as f64 + sign_position_offset.x);
        let zd = player_pos.z - (pos.z as f64 + sign_position_offset.z);
        let sign_y_rot = sign.get_y_rotation_degrees(state);
        let player_y_rot = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
        if mth::degrees_difference_abs(sign_y_rot, player_y_rot) <= 90.0 { SignTextSlot::Front } else { SignTextSlot::Back }
    }

    pub fn sign_update_sign_text(&mut self, pos: BlockPos, player: EntityId, slot: SignTextSlot, lines: &[FilteredText]) {
        let player_uuid = self.player(player).profile.uuid;
        let text_filtering_enabled = self.player(player).client.text_filtering;
        let Some(sign) = self.sign(pos) else { return };
        if !sign.is_waxed() && sign.get_player_who_may_edit() == Some(player_uuid) {
            let new_text = update_messages(lines, sign.get_text(slot), text_filtering_enabled);
            self.sign_update_text(pos, slot, Some(new_text));
            if let Some(sign) = self.sign(pos) {
                sign.set_allowed_player_editor(None);
            }
            if let Some(state) = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) {
                self.send_block_updated(pos, state, state);
            }
        } else {
            warn!("Player {} just tried to change non-editable sign", self.player(player).profile.name);
        }
    }

    pub fn sign_update_text(&mut self, pos: BlockPos, slot: SignTextSlot, new_text: Option<SignText>) -> bool {
        match new_text {
            Some(new_text) => {
                self.sign_set_text(pos, new_text, slot);
                true
            }
            None => false,
        }
    }

    pub fn sign_set_text(&mut self, pos: BlockPos, text: SignText, slot: SignTextSlot) {
        let Some(sign) = self.sign(pos) else { return };
        match slot {
            SignTextSlot::Front => sign.front_text = text,
            SignTextSlot::Back => sign.back_text = text,
        }
        self.sign_mark_updated(pos);
    }

    pub fn sign_execute_click_commands_if_present(&mut self, player: EntityId, pos: BlockPos, slot: SignTextSlot) -> bool {
        let text_filtering_enabled = self.player(player).client.text_filtering;
        let Some(sign) = self.sign(pos) else { return false };
        let allow_op_features = sign.allow_op_features;
        let has_any_click_command = sign
            .get_text(slot)
            .get_messages(text_filtering_enabled)
            .iter()
            .any(|message| matches!(message.style.click_event, Some(ClickEvent::RunCommand(_)) | Some(ClickEvent::Other { action: "show_dialog" | "custom", .. })));
        if !allow_op_features && has_any_click_command {
            let message = Component::translatable("sign.click_actions_disabled").with_color(ChatFormatting::Red);
            self.player(player).connection.send(crisp_data::packets::play::clientbound::SYSTEM_CHAT, |w| {
                message.write(w);
                w.bool(true);
            });
        }
        has_any_click_command
    }

    fn sign_mark_updated(&mut self, pos: BlockPos) {
        let Some(state) = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) else { return };
        self.block_entity_set_changed(pos);
        self.send_block_updated(pos, state, state);
    }

    pub fn sign_set_waxed(&mut self, pos: BlockPos, is_waxed: bool) -> bool {
        let Some(sign) = self.sign(pos) else { return false };
        if sign.is_waxed != is_waxed {
            sign.is_waxed = is_waxed;
            self.sign_mark_updated(pos);
            true
        } else {
            false
        }
    }

    fn sign_player_is_too_far_away_to_edit(&mut self, pos: BlockPos, player: u128) -> bool {
        match self.entity_manager.visible_entity_storage.get(player).filter(|&id| self.is_player(id)) {
            Some(editing_player) => !self.is_within_block_interaction_range(editing_player, pos, 4.0),
            None => true,
        }
    }

    pub fn sign_tick(&mut self, pos: BlockPos) {
        let Some(player_who_may_edit) = self.sign(pos).and_then(|sign| sign.get_player_who_may_edit()) else { return };
        if self.sign_player_is_too_far_away_to_edit(pos, player_who_may_edit)
            && let Some(sign) = self.sign(pos)
        {
            sign.set_allowed_player_editor(None);
        }
    }

    pub fn sign_get_sign_interaction_failed_sound_event(&self, pos: BlockPos) -> &'static str {
        match self.chunks.block_entity_ref(pos).map(|entity| entity.ty()) {
            Some(types::HANGING_SIGN) => "minecraft:block.hanging_sign.waxed_interact_fail",
            _ => "minecraft:block.sign.waxed_interact_fail",
        }
    }
}
