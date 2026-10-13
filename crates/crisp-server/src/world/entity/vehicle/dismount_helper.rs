use crisp_core::shapes::VoxelShape;
use crisp_core::{AABB, Axis, BlockPos, Direction, Vec3};
use crisp_data::{BlockState, block_tags, blocks};
use crisp_data::properties::OPEN;

use crate::server::level::ServerLevel;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::level::BlockGetter;
use crate::world::level::pathfinder::node_evaluator::is_burning_block;
use crate::world::phys::shapes::collision_context::CollisionContext;

#[derive(Clone, Copy)]
pub struct DismountType {
    pub dimensions: EntityDimensions,
    pub fire_immune: bool,
    pub is_player: bool,
}

pub fn is_block_dangerous(entity_type: DismountType, state: BlockState) -> bool {
    if !entity_type.fire_immune && is_burning_block(state) {
        return true;
    }
    state.is_block(blocks::WITHER_ROSE) || state.is_block(blocks::SWEET_BERRY_BUSH) || state.is_block(blocks::CACTUS) || state.is_block(blocks::POWDER_SNOW)
}

pub fn is_block_floor_valid(block_floor_height: f64) -> bool {
    !block_floor_height.is_infinite() && block_floor_height < 1.0
}

pub fn non_climbable_shape(level: &mut ServerLevel, pos: BlockPos) -> &'static VoxelShape {
    static EMPTY: std::sync::LazyLock<VoxelShape> = std::sync::LazyLock::new(VoxelShape::empty);
    let block_state = level.block_state(pos);
    if !block_state.is(&block_tags::CLIMBABLE) && !(block_state.block().is_instance_of("TrapDoorBlock") && block_state.get(OPEN)) {
        block_state.collision_voxel_shape()
    } else {
        &EMPTY
    }
}

pub fn offsets_for_direction(forward: Direction) -> [[i32; 2]; 8] {
    let right = forward.clockwise();
    let left = right.opposite();
    let back = forward.opposite();
    [
        [right.step_x(), right.step_z()],
        [left.step_x(), left.step_z()],
        [back.step_x() + right.step_x(), back.step_z() + right.step_z()],
        [back.step_x() + left.step_x(), back.step_z() + left.step_z()],
        [forward.step_x() + right.step_x(), forward.step_z() + right.step_z()],
        [forward.step_x() + left.step_x(), forward.step_z() + left.step_z()],
        [back.step_x(), back.step_z()],
        [forward.step_x(), forward.step_z()],
    ]
}

pub fn can_dismount_to(level: &ServerLevel, passenger: EntityId, box_: &AABB) -> bool {
    if !level.block_collisions(box_, &CollisionContext::of(level, passenger), false).iter().all(|(_, collider)| collider.shape.is_empty()) {
        return false;
    }
    level.world_border.is_within_bounds_aabb(box_)
}

pub fn can_dismount_to_pose(level: &ServerLevel, location: Vec3, passenger: EntityId, dismount_pose: Pose) -> bool {
    let bounds = level.local_bounds_for_pose(passenger, dismount_pose);
    can_dismount_to(level, passenger, &bounds.move_by(location.x, location.y, location.z))
}

pub fn find_ceiling_from(level: &mut ServerLevel, pos: BlockPos, blocks: i32) -> f64 {
    let mut cursor = pos;
    let mut y = 0;
    while y < blocks {
        let collision_shape = level.block_state(cursor).collision_voxel_shape();
        if !collision_shape.is_empty() {
            return (pos.y + y) as f64 + collision_shape.min(Axis::Y);
        }
        y += 1;
        cursor = cursor.above();
    }
    f64::INFINITY
}

pub fn get_block_floor_height_non_climbable(level: &mut ServerLevel, pos: BlockPos) -> f64 {
    let block_shape = non_climbable_shape(level, pos);
    if !block_shape.is_empty() {
        return block_shape.max(Axis::Y);
    }
    let below_floor = non_climbable_shape(level, pos.below()).max(Axis::Y);
    if below_floor >= 1.0 { below_floor - 1.0 } else { f64::NEG_INFINITY }
}

pub fn get_block_floor_height(level: &mut ServerLevel, pos: BlockPos) -> f64 {
    let block_shape = level.block_state(pos).collision_voxel_shape();
    if !block_shape.is_empty() {
        return block_shape.max(Axis::Y);
    }
    let below_floor = level.block_state(pos.below()).collision_voxel_shape().max(Axis::Y);
    if below_floor >= 1.0 { below_floor - 1.0 } else { f64::NEG_INFINITY }
}

pub fn find_safe_dismount_location(entity_type: DismountType, level: &mut ServerLevel, block_pos: BlockPos, check_dangerous: bool) -> Option<Vec3> {
    if check_dangerous && is_block_dangerous(entity_type, level.block_state(block_pos)) {
        return None;
    }
    let floor_height = get_block_floor_height_non_climbable(level, block_pos);
    if !is_block_floor_valid(floor_height) {
        return None;
    }
    if check_dangerous && floor_height <= 0.0 && is_block_dangerous(entity_type, level.block_state(block_pos.below())) {
        return None;
    }
    let position = Vec3::new(block_pos.x as f64 + 0.5, block_pos.y as f64 + floor_height, block_pos.z as f64 + 0.5);
    let aabb = entity_type.dimensions.make_bounding_box(position);
    if !level.block_collisions(&aabb, &CollisionContext::EMPTY, false).iter().all(|(_, collider)| collider.shape.is_empty()) {
        return None;
    }
    if entity_type.is_player && (level.block_state(block_pos).is(&block_tags::INVALID_SPAWN_INSIDE) || level.block_state(block_pos.above()).is(&block_tags::INVALID_SPAWN_INSIDE)) {
        return None;
    }
    if !level.world_border.is_within_bounds_aabb(&aabb) { None } else { Some(position) }
}
