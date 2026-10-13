use crisp_core::BlockPos;
use crisp_data::{registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::ItemStack;
use super::component as dc;
use super::dye_color::DyeColor;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::entity::animal::sheep::sheep;
use crate::world::level::block::entity::sign_block_entity::SignTextSlot;

pub fn interact_living_entity(level: &mut ServerLevel, item_stack: &mut ItemStack, player: EntityId, target: EntityId) -> InteractionResult {
    if level.is_sheep(target) && level.is_alive(target) && !sheep::is_sheared(level.entity(target)) {
        let dye_color = item_stack.get(dc::DYE).and_then(Nbt::as_str).and_then(DyeColor::by_name);
        if let Some(dye_color) = dye_color
            && sheep::get_color(level.entity(target)) != dye_color
        {
            level.play_sound_from_entity(Some(player), target, registry_id(registry::SOUND_EVENT, "minecraft:item.dye.use"), SoundSource::Players, 1.0, 1.0);
            sheep::set_color(level.entity_mut(target), dye_color);
            item_stack.shrink(1);
            return InteractionResult::SUCCESS;
        }
    }
    InteractionResult::Pass
}

pub fn try_apply_to_sign(level: &mut ServerLevel, pos: BlockPos, slot: SignTextSlot, item: &ItemStack) -> bool {
    let Some(dye) = item.get(dc::DYE).and_then(Nbt::as_str).and_then(DyeColor::by_name) else { return false };
    let new_text = level.sign(pos).and_then(|sign| sign.get_text(slot).with_color(dye));
    if level.sign_update_text(pos, slot, new_text) {
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:item.dye.use"), SoundSource::Blocks, 1.0, 1.0);
        true
    } else {
        false
    }
}
