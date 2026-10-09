use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{BlockState, blocks, registry, registry_id};

use super::BlockBehaviour;
use crate::server::level::events::PARTICLES_WATER_EVAPORATING;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct WetSpongeBlock;

impl BlockBehaviour for WetSpongeBlock {
    fn on_place(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        if level.environment_attributes().get_value_at(environment_attributes::WATER_EVAPORATES, pos) {
            level.set_block(pos, blocks::SPONGE.default_state(), UPDATE_ALL);
            level.level_event(None, PARTICLES_WATER_EVAPORATING, pos, 0);
            let sound = registry_id(registry::SOUND_EVENT, "minecraft:block.wet_sponge.dries");
            let pitch = (1.0 + level.random.next_float() * 0.2) * 0.7;
            level.play_sound(None, pos, sound, SoundSource::Blocks, 1.0, pitch);
        }
    }
}
