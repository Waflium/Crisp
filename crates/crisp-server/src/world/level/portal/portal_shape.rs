use crisp_core::{AABB, Axis, BlockPos, Direction, Vec3, mth};
use crisp_data::properties::HORIZONTAL_AXIS;
use crisp_data::{BlockState, block_tags, blocks};

use crate::server::level::ServerLevel;
use crate::util::block_util::FoundRectangle;
use crate::world::entity::EntityDimensions;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::phys::shapes::collision_context::CollisionContext;

const MIN_WIDTH: i32 = 2;
pub const MAX_WIDTH: i32 = 21;
const MIN_HEIGHT: i32 = 3;
pub const MAX_HEIGHT: i32 = 21;
const SAFE_TRAVEL_MAX_ENTITY_XY: f32 = 4.0;

pub fn frame(state: BlockState) -> bool {
    state.is(&block_tags::NETHER_PORTAL_FRAME)
}

#[derive(Clone, Copy, Debug)]
pub struct PortalShape {
    axis: Axis,
    right_dir: Direction,
    num_portal_blocks: i32,
    bottom_left: BlockPos,
    height: i32,
    width: i32,
}

impl PortalShape {
    pub fn find_empty_portal_shape(level: &mut dyn LevelAccessor, pos: BlockPos, preferred_axis: Axis) -> Option<PortalShape> {
        PortalShape::find_portal_shape(level, pos, |shape| shape.is_valid() && shape.num_portal_blocks == 0, preferred_axis)
    }

    pub fn find_portal_shape(level: &mut dyn LevelAccessor, pos: BlockPos, is_valid: impl Fn(&PortalShape) -> bool, preferred_axis: Axis) -> Option<PortalShape> {
        let first_axis = Some(PortalShape::find_any_shape(level, pos, preferred_axis)).filter(&is_valid);
        if first_axis.is_some() {
            return first_axis;
        }
        let other_axis = if preferred_axis == Axis::X { Axis::Z } else { Axis::X };
        Some(PortalShape::find_any_shape(level, pos, other_axis)).filter(&is_valid)
    }

    pub fn find_any_shape(level: &mut dyn BlockGetter, pos: BlockPos, axis: Axis) -> PortalShape {
        let right_dir = if axis == Axis::X { Direction::West } else { Direction::South };
        let Some(bottom_left) = calculate_bottom_left(level, right_dir, pos) else {
            return PortalShape { axis, num_portal_blocks: 0, right_dir, bottom_left: pos, width: 0, height: 0 };
        };
        let width = calculate_width(level, bottom_left, right_dir);
        if width == 0 {
            return PortalShape { axis, num_portal_blocks: 0, right_dir, bottom_left, width: 0, height: 0 };
        }
        let mut portal_block_count_output = 0;
        let height = calculate_height(level, bottom_left, right_dir, width, &mut portal_block_count_output);
        PortalShape { axis, num_portal_blocks: portal_block_count_output, right_dir, bottom_left, width, height }
    }

    pub fn is_valid(&self) -> bool {
        self.width >= MIN_WIDTH && self.width <= MAX_WIDTH && self.height >= MIN_HEIGHT && self.height <= MAX_HEIGHT
    }

    pub fn create_portal_blocks(&self, level: &mut dyn LevelAccessor) {
        let portal_state = blocks::NETHER_PORTAL.default_state().set(HORIZONTAL_AXIS, self.axis);
        let top_right = self.bottom_left.relative_n(Direction::Up, self.height - 1).relative_n(self.right_dir, self.width - 1);
        for pos in BlockPos::between_closed(self.bottom_left, top_right) {
            level.set_block(pos, portal_state, 18);
        }
    }

    pub fn is_complete(&self) -> bool {
        self.is_valid() && self.num_portal_blocks == self.width * self.height
    }
}

fn calculate_bottom_left(level: &mut dyn BlockGetter, right_dir: Direction, mut pos: BlockPos) -> Option<BlockPos> {
    let min_y = level.get_min_y().max(pos.y - MAX_HEIGHT);
    while pos.y > min_y && is_empty(level.block_state(pos.below())) {
        pos = pos.below();
    }
    let left_dir = right_dir.opposite();
    let edge = get_distance_until_edge_above_frame(level, pos, left_dir) - 1;
    if edge < 0 { None } else { Some(pos.relative_n(left_dir, edge)) }
}

fn calculate_width(level: &mut dyn BlockGetter, bottom_left: BlockPos, right_dir: Direction) -> i32 {
    let width = get_distance_until_edge_above_frame(level, bottom_left, right_dir);
    if (MIN_WIDTH..=MAX_WIDTH).contains(&width) { width } else { 0 }
}

fn get_distance_until_edge_above_frame(level: &mut dyn BlockGetter, pos: BlockPos, direction: Direction) -> i32 {
    for width in 0..=MAX_WIDTH {
        let block_pos = pos.relative_n(direction, width);
        let block_state = level.block_state(block_pos);
        if !is_empty(block_state) {
            if frame(block_state) {
                return width;
            }
            break;
        }
        let below_state = level.block_state(block_pos.below());
        if !frame(below_state) {
            break;
        }
    }
    0
}

fn calculate_height(level: &mut dyn BlockGetter, bottom_left: BlockPos, right_dir: Direction, width: i32, portal_block_count: &mut i32) -> i32 {
    let height = get_distance_until_top(level, bottom_left, right_dir, width, portal_block_count);
    if (MIN_HEIGHT..=MAX_HEIGHT).contains(&height) && has_top_frame(level, bottom_left, right_dir, width, height) { height } else { 0 }
}

fn has_top_frame(level: &mut dyn BlockGetter, bottom_left: BlockPos, right_dir: Direction, width: i32, height: i32) -> bool {
    (0..width).all(|i| frame(level.block_state(bottom_left.relative_n(Direction::Up, height).relative_n(right_dir, i))))
}

fn get_distance_until_top(level: &mut dyn BlockGetter, bottom_left: BlockPos, right_dir: Direction, width: i32, portal_block_count: &mut i32) -> i32 {
    for height in 0..MAX_HEIGHT {
        let row = bottom_left.relative_n(Direction::Up, height);
        if !frame(level.block_state(row.relative_n(right_dir, -1))) {
            return height;
        }
        if !frame(level.block_state(row.relative_n(right_dir, width))) {
            return height;
        }
        for i in 0..width {
            let state = level.block_state(row.relative_n(right_dir, i));
            if !is_empty(state) {
                return height;
            }
            if state.is_block(blocks::NETHER_PORTAL) {
                *portal_block_count += 1;
            }
        }
    }
    MAX_HEIGHT
}

fn is_empty(state: BlockState) -> bool {
    state.is_air() || state.is(&block_tags::FIRE) || state.is_block(blocks::NETHER_PORTAL)
}

pub fn get_relative_position(largest_rectangle_around: FoundRectangle, axis: Axis, position: Vec3, dimensions: EntityDimensions) -> Vec3 {
    let width = largest_rectangle_around.axis1_size as f64 - dimensions.width as f64;
    let height = largest_rectangle_around.axis2_size as f64 - dimensions.height as f64;
    let bottom_min = largest_rectangle_around.min_corner;
    let relative_right = if width > 0.0 {
        let bottom_start = bottom_min.get(axis) as f64 + dimensions.width as f64 / 2.0;
        mth::clamp(mth::inverse_lerp(position.get(axis) - bottom_start, 0.0, width), 0.0, 1.0)
    } else {
        0.5
    };
    let relative_up = if height > 0.0 {
        let height_axis = Axis::Y;
        mth::clamp(mth::inverse_lerp(position.get(height_axis) - bottom_min.get(height_axis) as f64, 0.0, height), 0.0, 1.0)
    } else {
        0.0
    };
    let forward_axis = if axis == Axis::X { Axis::Z } else { Axis::X };
    let relative_forward = position.get(forward_axis) - (bottom_min.get(forward_axis) as f64 + 0.5);
    Vec3::new(relative_right, relative_up, relative_forward)
}

pub fn find_collision_free_position(bottom_center: Vec3, server_level: &ServerLevel, context: &CollisionContext, dimensions: EntityDimensions) -> Vec3 {
    if dimensions.width > SAFE_TRAVEL_MAX_ENTITY_XY || dimensions.height > SAFE_TRAVEL_MAX_ENTITY_XY {
        return bottom_center;
    }
    let half_height = dimensions.height as f64 / 2.0;
    let center = bottom_center.add(0.0, half_height, 0.0);
    let width = dimensions.width as f64;
    let allowed_centers = AABB::of_size(center, width, 0.0, width).expand_towards(0.0, 1.0, 0.0).inflate(1.0e-6, 1.0e-6, 1.0e-6);
    let collision_free_position = server_level.find_free_position(context, &allowed_centers, center, width, dimensions.height as f64, width);
    collision_free_position.map(|vec| vec.add(-0.0, -half_height, -0.0)).unwrap_or(bottom_center)
}
