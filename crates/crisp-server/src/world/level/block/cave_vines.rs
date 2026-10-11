use crisp_core::{BlockPos, mth};
use crisp_data::properties::BERRIES;
use crisp_data::{BlockState, registry, registry_id};

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::level::block;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub fn use_vine(source_entity: Option<EntityId>, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> InteractionResult {
    if !state.get(BERRIES) {
        return InteractionResult::Pass;
    }
    let block_entity = level.get_block_entity(pos).cloned();
    block::drop_from_block_interact_loot_table(level, "minecraft:harvest/cave_vine", pos, state, block_entity.as_ref(), None, source_entity, |level, stack| {
        level.pop_resource(pos, stack)
    });
    let pitch = mth::random_between(&mut level.random, 0.8, 1.2);
    let sound = registry_id(registry::SOUND_EVENT, "minecraft:block.cave_vines.pick_berries");
    level.play_sound(None, pos, sound, SoundSource::Blocks, 1.0, pitch);
    level.set_block(pos, state.set(BERRIES, false), UPDATE_CLIENTS);
    InteractionResult::SUCCESS
}
