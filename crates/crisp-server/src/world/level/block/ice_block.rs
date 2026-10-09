use crisp_core::BlockPos;
use crisp_data::{BlockState, block_tags, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct IceBlock;

pub fn melts_into() -> BlockState {
    blocks::WATER.default_state()
}

pub fn melt(level: &mut ServerLevel, pos: BlockPos) {
    if level.environment_attributes().get_value_at(environment_attributes::WATER_EVAPORATES, pos) {
        level.remove_block(pos, false);
    } else {
        level.set_block(pos, melts_into(), UPDATE_ALL);
        level.neighbor_changed(pos, melts_into().block());
    }
}

impl BlockBehaviour for IceBlock {
    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, state: BlockState, _block_entity: Option<&crate::world::level::block::entity::BlockEntity>, tool: Option<ItemStack>) {
        level.drop_resources(state, pos, Some(player), tool);
        if level.environment_attributes().get_value_at(environment_attributes::WATER_EVAPORATES, pos) {
            level.remove_block(pos, false);
            return;
        }
        let below = level.block_state(pos.below());
        if below.is(&block_tags::ICE_MELTS_WHEN_DESTROYED_ABOVE) || below.liquid() {
            level.set_block(pos, melts_into(), UPDATE_ALL);
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.brightness(LightLayer::Block, pos) > 11 - state.light_dampening() as i32 {
            melt(level, pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::blocks;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn ice_melts_next_to_block_light_only() {
        let mut level = testing::level();
        let ice = blocks::ICE.default_state();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, ice, UPDATE_ALL);
        block::of(ice).random_tick(ice, &mut level, pos);
        assert_eq!(level.block_state(pos), ice);
        level.set_block(pos.east(), blocks::GLOWSTONE.default_state(), UPDATE_ALL);
        level.chunks.run_light_updates();
        block::of(ice).random_tick(ice, &mut level, pos);
        assert_eq!(level.block_state(pos), blocks::WATER.default_state());
    }

    #[test]
    fn ice_broken_above_liquid_leaves_water() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 30.0);
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos.below(), blocks::WATER.default_state(), UPDATE_ALL);
        level.set_block(pos, blocks::ICE.default_state(), UPDATE_ALL);
        level.destroy_block_by_player(player, pos);
        assert_eq!(level.block_state(pos), blocks::WATER.default_state());
        let dry = BlockPos::new(1, -58, 0);
        level.set_block(dry, blocks::ICE.default_state(), UPDATE_ALL);
        level.destroy_block_by_player(player, dry);
        assert!(level.block_state(dry).is_air());
    }
}
