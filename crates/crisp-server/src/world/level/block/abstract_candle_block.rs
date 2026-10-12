use crisp_core::BlockPos;
use crisp_data::properties::LIT;
use crisp_data::{BlockState, block_tags, registry, registry_id};

use crate::server::level::{ServerLevel, SoundSource};

pub fn is_lit(state: BlockState) -> bool {
    state.has(LIT) && (state.is(&block_tags::CANDLES) || state.is(&block_tags::CANDLE_CAKES)) && state.get(LIT)
}

pub fn extinguish(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    set_lit(level, state, pos, false);
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.candle.extinguish"), SoundSource::Blocks, 1.0, 1.0);
}

pub fn set_lit(level: &mut ServerLevel, state: BlockState, pos: BlockPos, lit: bool) {
    level.set_block(pos, state.set(LIT, lit), 11);
}
