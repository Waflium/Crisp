use crisp_core::{BlockPos, Direction};
use crisp_data::properties::ROTATION_16;
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::abstract_banner_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::rotation_segment;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct BannerBlock;

impl BlockBehaviour for BannerBlock {
    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.below()).is_solid()
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(ROTATION_16, rotation_segment::convert_to_segment(ctx.rotation() + 180.0)))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Down && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ROTATION_16, rotation.rotate_index(state.get(ROTATION_16), 16))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ROTATION_16, mirror.mirror_index(state.get(ROTATION_16), 16))
    }

    fn get_clone_item_stack(&self, level: &ServerLevel, pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        abstract_banner_block::get_clone_item_stack(level, pos, state, include_data)
    }
}
