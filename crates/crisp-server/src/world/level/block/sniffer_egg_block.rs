use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::properties::HATCH;
use crisp_data::{Block, BlockState, block_tags, registry, registry_id};

use super::BlockBehaviour;
use crate::server::level::events::PARTICLES_EGG_CRACK;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_LIMIT};
use crate::world::level::BlockGetter;

pub const MAX_HATCH_LEVEL: i32 = 2;
const REGULAR_HATCH_TIME_TICKS: i32 = 24000;
const BOOSTED_HATCH_TIME_TICKS: i32 = 12000;
const RANDOM_HATCH_OFFSET_TICKS: i32 = 300;

pub fn hatch_boost(level: &mut ServerLevel, pos: BlockPos) -> bool {
    level.block_state(pos.below()).is(&block_tags::SNIFFER_EGG_HATCH_BOOST)
}

pub struct SnifferEggBlock {
    block: Block,
}

impl SnifferEggBlock {
    pub fn new(block: Block) -> SnifferEggBlock {
        SnifferEggBlock { block }
    }

    fn get_hatch_level(&self, state: BlockState) -> i32 {
        state.get(HATCH)
    }

    fn is_ready_to_hatch(&self, state: BlockState) -> bool {
        self.get_hatch_level(state) == MAX_HATCH_LEVEL
    }
}

impl BlockBehaviour for SnifferEggBlock {
    fn tick(&self, state: BlockState, level: &mut ServerLevel, position: BlockPos) {
        let sound = |name: &str| registry_id(registry::SOUND_EVENT, name);
        if !self.is_ready_to_hatch(state) {
            let pitch = 0.9 + level.random.next_float() * 0.2;
            level.play_sound(None, position, sound("minecraft:block.sniffer_egg.crack"), SoundSource::Blocks, 0.7, pitch);
            level.set_block(position, state.set(HATCH, self.get_hatch_level(state) + 1), UPDATE_CLIENTS);
        } else {
            let pitch = 0.9 + level.random.next_float() * 0.2;
            level.play_sound(None, position, sound("minecraft:block.sniffer_egg.hatch"), SoundSource::Blocks, 0.7, pitch);
            level.destroy_block(position, false, None, UPDATE_LIMIT);
            if let Some(mut sniffer) = ServerLevel::create_entity("minecraft:sniffer") {
                let spawn_at = Vec3::at_center_of(position);
                let y_rot = mth::wrap_degrees(level.random.next_float() * 360.0);
                sniffer.entity_mut().snap_to(spawn_at.x, spawn_at.y, spawn_at.z, y_rot, 0.0);
                level.add_fresh_entity(sniffer);
            }
        }
    }

    fn on_place(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        let boosted = hatch_boost(level, pos);
        if boosted {
            level.level_event(None, PARTICLES_EGG_CRACK, pos, 0);
        }
        let hatch_time = if boosted { BOOSTED_HATCH_TIME_TICKS } else { REGULAR_HATCH_TIME_TICKS };
        let progression_tick_delay = hatch_time / 3;
        let delay = progression_tick_delay + level.random.next_int_bounded(RANDOM_HATCH_OFFSET_TICKS);
        level.schedule_tick(pos, self.block, delay);
    }
}
