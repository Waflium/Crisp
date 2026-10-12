use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{registry, registry_id};

use super::container_openers_counter::ContainerOpenersCounter;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;

#[derive(Clone, Default)]
pub struct EnderChestBlockEntity {
    pub openers_counter: ContainerOpenersCounter,
}

impl ServerLevel {
    pub fn ender_chest_start_open(&mut self, pos: BlockPos, player: EntityId) {
        if self.get_block_entity(pos).is_some_and(|entity| !entity.is_removed()) && !self.is_spectator(player) {
            self.increment_openers(pos, crate::server::level::game_mode::BLOCK_INTERACTION_RANGE);
        }
    }

    pub fn ender_chest_stop_open(&mut self, pos: BlockPos, player: EntityId) {
        if self.get_block_entity(pos).is_some_and(|entity| !entity.is_removed()) && !self.is_spectator(player) {
            self.decrement_openers(pos);
        }
    }

    pub fn ender_chest_recheck_open(&mut self, pos: BlockPos) {
        if self.get_block_entity(pos).is_some_and(|entity| !entity.is_removed()) {
            self.recheck_openers(pos);
        }
    }

    pub fn ender_chest_play_sound(&mut self, pos: BlockPos, sound: &str) {
        let pitch = self.random.next_float() * 0.1 + 0.9;
        let sound = registry_id(registry::SOUND_EVENT, sound);
        self.play_sound_at(None, pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5, sound, SoundSource::Blocks, 0.5, pitch);
    }
}
