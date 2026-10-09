use crisp_core::shapes::{VoxelShape, is_face_full};
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::LAYERS;
use crisp_data::{Block, BlockState, block_tags, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::light_layer::LightLayer;

pub struct SnowLayerBlock;

impl BlockBehaviour for SnowLayerBlock {
    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let below = level.block_state(pos.below());
        if below.is(&block_tags::CANNOT_SUPPORT_SNOW_LAYER) {
            false
        } else if below.is(&block_tags::SUPPORT_OVERRIDE_SNOW_LAYER) {
            true
        } else {
            is_face_full(&VoxelShape::from_boxes(below.collision_shape()), Direction::Up) || below.is_block(blocks::SNOW) && below.get(LAYERS) == 8
        }
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if block::of(state).can_survive(state, level, pos) { state } else { blocks::AIR.default_state() }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.brightness(LightLayer::Block, pos) > 11 {
            level.drop_resources(state, pos, None, None);
            level.remove_block(pos, false);
        }
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        let layers = state.get(LAYERS);
        if ctx.item.item != state.block().item() || layers >= 8 {
            layers == 1
        } else if ctx.replacing_clicked_on_block() {
            ctx.clicked_face() == Direction::Up
        } else {
            true
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let state = level.block_state(ctx.clicked_pos());
        if state.is_block(block) {
            let layers = state.get(LAYERS);
            Some(state.set(LAYERS, 8.min(layers + 1)))
        } else {
            Some(block.default_state())
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::LAYERS;
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::{self, click};
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn stacks_and_needs_support() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::SNOW, 64));
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        click(&mut level, player, pos, Direction::Up, Vec3::new(0.5, -59.875, 0.5));
        assert_eq!(level.block_state(pos).get(LAYERS), 2);

        let ice = BlockPos::new(3, -61, 0);
        level.set_block(ice, blocks::ICE.default_state(), UPDATE_ALL);
        click(&mut level, player, ice, Direction::Up, Vec3::new(3.5, -60.0, 0.5));
        assert!(level.block_state(ice.above()).is_air());

        level.set_block(pos.below(), blocks::AIR.default_state(), UPDATE_ALL);
        assert!(level.block_state(pos).is_air());
    }

    #[test]
    fn melts_next_to_a_light() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::SNOW.default_state(), UPDATE_ALL);
        let snow = level.block_state(pos);
        block::of(snow).random_tick(snow, &mut level, pos);
        assert!(level.block_state(pos).is_block(blocks::SNOW));
        level.set_block(pos.east(), blocks::GLOWSTONE.default_state(), UPDATE_ALL);
        level.chunks.run_light_updates();
        block::of(snow).random_tick(snow, &mut level, pos);
        assert!(level.block_state(pos).is_air());
    }
}
