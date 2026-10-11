use crisp_core::BlockPos;
use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::AABB;
use crisp_data::properties::LIT;
use crisp_data::{BlockState, block_tags};

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

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

pub fn is_lit_campfire(block_state: BlockState) -> bool {
    block_state.has(LIT) && block_state.is(&block_tags::CAMPFIRES) && block_state.get(LIT)
}
