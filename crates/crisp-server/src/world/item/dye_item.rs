use crisp_core::BlockPos;
use crisp_data::{registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::ItemStack;
use super::component as dc;
use super::dye_color::DyeColor;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::level::block::entity::sign_block_entity::SignTextSlot;

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
