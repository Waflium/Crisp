use crisp_core::BlockPos;
use crisp_data::{registry, registry_id};

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::level::block::entity::sign_block_entity::SignTextSlot;

pub fn try_apply_to_sign(level: &mut ServerLevel, pos: BlockPos, slot: SignTextSlot) -> bool {
    let new_text = level.sign(pos).and_then(|sign| sign.get_text(slot).with_glowing_text(false));
    if level.sign_update_text(pos, slot, new_text) {
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:item.ink_sac.use"), SoundSource::Blocks, 1.0, 1.0);
        true
    } else {
        false
    }
}
