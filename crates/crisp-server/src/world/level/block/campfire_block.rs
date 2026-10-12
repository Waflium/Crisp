use crisp_core::BlockPos;
use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::AABB;
use crisp_data::properties::{HORIZONTAL_FACING, LIT, WATERLOGGED};
use crisp_data::{BlockState, block_tags};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

pub struct CampfireBlock;

impl BlockBehaviour for CampfireBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}

fn shape_virtual_post() -> VoxelShape {
    VoxelShape::from_boxes(&[AABB::new(0.375, 0.0, 0.375, 0.625, 1.0, 0.625)])
}

pub fn is_smokey_pos(level: &mut ServerLevel, pos: BlockPos) -> bool {
    for i in 1..=5 {
        let pos_to_check = pos.offset(0, -i, 0);
        let block_state = level.block_state(pos_to_check);
        if is_lit_campfire(block_state) {
            return true;
        }
        let smoke_blocked = join_is_not_empty(&shape_virtual_post(), &VoxelShape::from_boxes(block_state.collision_shape()), BooleanOp::And);
        if smoke_blocked {
            let below_state = level.block_state(pos_to_check.below());
            return is_lit_campfire(below_state);
        }
    }
    false
}

pub fn can_light(state: BlockState) -> bool {
    state.is(&block_tags::CAMPFIRES) && state.has(WATERLOGGED) && state.has(LIT) && !state.get(WATERLOGGED) && !state.get(LIT)
}

pub fn is_lit_campfire(block_state: BlockState) -> bool {
    block_state.has(LIT) && block_state.is(&block_tags::CAMPFIRES) && block_state.get(LIT)
}
