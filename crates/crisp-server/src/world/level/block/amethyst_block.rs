use crisp_core::random::RandomSource;
use crisp_data::{BlockState, registry, registry_id};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;

pub struct AmethystBlock;

pub fn on_projectile_hit(level: &mut ServerLevel, hit_result: &BlockHitResult) {
    let hit_pos = hit_result.block_pos;
    let pitch = 0.5 + level.random.next_float() * 1.2;
    level.play_sound(None, hit_pos, registry_id(registry::SOUND_EVENT, "minecraft:block.amethyst_block.chime"), SoundSource::Blocks, 1.0, pitch);
}

impl BlockBehaviour for AmethystBlock {
    fn on_projectile_hit(&self, level: &mut ServerLevel, _state: BlockState, hit_result: &BlockHitResult, _projectile: EntityId) {
        on_projectile_hit(level, hit_result);
    }
}
