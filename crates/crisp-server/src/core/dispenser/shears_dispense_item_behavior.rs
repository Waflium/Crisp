use crisp_core::{AABB, BlockPos};
use crisp_data::properties::LEVEL_HONEY;
use crisp_data::{block_tags, registry, registry_id};

use super::block_source::BlockSource;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::beehive_block;
use crate::world::level::block::entity::beehive_block_entity::BeeReleaseStatus;

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, success: &mut bool) -> ItemStack {
    let pos = source.pos.relative(source.facing());
    *success = try_shear_beehive(level, &dispensed, pos) || try_shear_entity(level, pos, &dispensed);
    if *success {
        dispensed.hurt_and_break(1, level);
    }
    dispensed
}

fn try_shear_beehive(level: &mut ServerLevel, tool: &ItemStack, pos: BlockPos) -> bool {
    let state = level.block_state(pos);
    if state.is(&block_tags::BEEHIVES) && state.has(LEVEL_HONEY) && state.block().is_instance_of("BeehiveBlock") && state.get(LEVEL_HONEY) >= 5 {
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.beehive.shear"), SoundSource::Blocks, 1.0, 1.0);
        beehive_block::drop_honeycomb(level, tool, state, None, pos);
        beehive_block::release_bees_and_reset_honey_level(level, state, pos, None, BeeReleaseStatus::BeeReleased);
        return true;
    }
    false
}

fn try_shear_entity(level: &mut ServerLevel, pos: BlockPos, tool: &ItemStack) -> bool {
    for entity in level.get_entities(None, &AABB::of_block(pos)) {
        if level.is_alive(entity) && level.is_shearable(entity) && level.ready_for_shearing(entity) {
            level.shear(entity, SoundSource::Blocks, tool);
            return true;
        }
    }
    false
}
