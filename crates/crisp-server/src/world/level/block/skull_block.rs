use crisp_core::BlockPos;
use crisp_data::properties::ROTATION_16;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::abstract_skull_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::rotation_segment;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;

const MAX: i32 = 15;
const ROTATIONS: i32 = MAX + 1;

pub struct SkullBlock;

impl BlockBehaviour for SkullBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(abstract_skull_block::get_state_for_placement(block, ctx, level).set(ROTATION_16, rotation_segment::convert_to_segment(ctx.rotation())))
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        abstract_skull_block::neighbor_changed(state, level, pos);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ROTATION_16, rotation.rotate_index(state.get(ROTATION_16), ROTATIONS))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ROTATION_16, mirror.mirror_index(state.get(ROTATION_16), ROTATIONS))
    }
}
