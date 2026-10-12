use crisp_core::{Axis, AxisDirection, BlockPos, Direction, mth};
use crisp_data::blocks;
use crisp_data::properties::HORIZONTAL_AXIS;

use crate::server::level::ServerLevel;
use crate::util::block_util::FoundRectangle;
use crate::world::entity::ai::village::poi::poi_manager::{self, Occupancy};
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::BlockGetter;

const NETHER_PORTAL_RADIUS: i32 = 16;
const OVERWORLD_PORTAL_RADIUS: i32 = 128;

pub fn find_closest_portal_position(level: &mut ServerLevel, approximate_exit_pos: BlockPos, to_nether: bool) -> Option<BlockPos> {
    let radius = if to_nether { NETHER_PORTAL_RADIUS } else { OVERWORLD_PORTAL_RADIUS };
    poi_manager::ensure_loaded_and_valid(&mut level.chunks, approximate_exit_pos, radius);
    let records = level.chunks.poi_manager.get_in_square(&|poi_type| poi_type == PoiType::NetherPortal, approximate_exit_pos, radius, Occupancy::Any);
    let mut closest: Option<BlockPos> = None;
    for pos in records.into_iter().map(|record| record.pos) {
        if !level.world_border.is_within_bounds_pos(pos) || !level.block_state(pos).has(HORIZONTAL_AXIS) {
            continue;
        }
        let better = closest.is_none_or(|best| {
            let (distance, best_distance) = (pos.dist_sqr(approximate_exit_pos), best.dist_sqr(approximate_exit_pos));
            distance < best_distance || distance == best_distance && pos.y < best.y
        });
        if better {
            closest = Some(pos);
        }
    }
    closest
}

pub fn create_portal(level: &mut ServerLevel, origin: BlockPos, portal_axis: Axis) -> Option<FoundRectangle> {
    let direction = Direction::from_axis_and_direction(portal_axis, AxisDirection::Positive);
    let mut closest_full_distance_sqr = -1.0;
    let mut closest_full_position: Option<BlockPos> = None;
    let mut closest_partial_distance_sqr = -1.0;
    let mut closest_partial_position: Option<BlockPos> = None;
    let min_y = level.get_min_y();
    let max_placeable_y = level.chunks.max_y().min(min_y + level.dimension_type().logical_height as i32 - 1);
    for column in BlockPos::spiral_around(origin, 16, Direction::East, Direction::South) {
        let height = max_placeable_y.min(level.get_height(HeightmapType::MotionBlocking, column.x, column.z));
        if !level.world_border.is_within_bounds_pos(column) || !level.world_border.is_within_bounds_pos(column.relative(direction)) {
            continue;
        }
        let mut y = height;
        while y >= min_y {
            let mut column_pos = BlockPos::new(column.x, y, column.z);
            if can_portal_replace_block(level, column_pos) {
                let first_empty_y = y;
                while y > min_y && {
                    column_pos = column_pos.below();
                    can_portal_replace_block(level, column_pos)
                } {
                    y -= 1;
                }
                if y + 4 <= max_placeable_y {
                    let delta_y = first_empty_y - y;
                    if delta_y <= 0 || delta_y >= 3 {
                        let column_pos = BlockPos::new(column.x, y, column.z);
                        if can_host_frame(level, column_pos, direction, 0) {
                            let distance = origin.dist_sqr(column_pos);
                            if can_host_frame(level, column_pos, direction, -1)
                                && can_host_frame(level, column_pos, direction, 1)
                                && (closest_full_distance_sqr == -1.0 || closest_full_distance_sqr > distance)
                            {
                                closest_full_distance_sqr = distance;
                                closest_full_position = Some(column_pos);
                            }
                            if closest_full_distance_sqr == -1.0 && (closest_partial_distance_sqr == -1.0 || closest_partial_distance_sqr > distance) {
                                closest_partial_distance_sqr = distance;
                                closest_partial_position = Some(column_pos);
                            }
                        }
                    }
                }
            }
            y -= 1;
        }
    }
    if closest_full_distance_sqr == -1.0 && closest_partial_distance_sqr != -1.0 {
        closest_full_position = closest_partial_position;
        closest_full_distance_sqr = closest_partial_distance_sqr;
    }
    let closest_full_position = if closest_full_distance_sqr == -1.0 {
        let min_start_y = (min_y + 1).max(70);
        let max_start_y = max_placeable_y - 9;
        if max_start_y < min_start_y {
            return None;
        }
        let position = BlockPos::new(origin.x - direction.step_x(), mth::clamp_i(origin.y, min_start_y, max_start_y), origin.z - direction.step_z());
        let position = level.world_border.clamp_to_bounds(position.x as f64, position.y as f64, position.z as f64);
        let clock_wise = direction.clockwise();
        for box_ in -1..2 {
            for width in 0..2 {
                for height in -1..3 {
                    let block_state = if height < 0 { blocks::OBSIDIAN.default_state() } else { blocks::AIR.default_state() };
                    let pos = position.offset(
                        width * direction.step_x() + box_ * clock_wise.step_x(),
                        height,
                        width * direction.step_z() + box_ * clock_wise.step_z(),
                    );
                    level.set_block(pos, block_state, UPDATE_ALL);
                }
            }
        }
        position
    } else {
        closest_full_position.expect("a portal position")
    };
    for width in -1..3 {
        for height in -1..4 {
            if width == -1 || width == 2 || height == -1 || height == 3 {
                let pos = closest_full_position.offset(width * direction.step_x(), height, width * direction.step_z());
                level.set_block(pos, blocks::OBSIDIAN.default_state(), UPDATE_ALL);
            }
        }
    }
    let portal_block_state = blocks::NETHER_PORTAL.default_state().set(HORIZONTAL_AXIS, portal_axis);
    for width in 0..2 {
        for height in 0..3 {
            let pos = closest_full_position.offset(width * direction.step_x(), height, width * direction.step_z());
            level.set_block(pos, portal_block_state, 18);
        }
    }
    Some(FoundRectangle { min_corner: closest_full_position, axis1_size: 2, axis2_size: 3 })
}

fn can_portal_replace_block(level: &mut ServerLevel, pos: BlockPos) -> bool {
    let block_state = level.block_state(pos);
    block_state.can_be_replaced() && block_state.fluid_state().is_empty()
}

fn can_host_frame(level: &mut ServerLevel, origin: BlockPos, direction: Direction, offset: i32) -> bool {
    let clock_wise = direction.clockwise();
    for width in -1..3 {
        for height in -1..4 {
            let pos = origin.offset(
                direction.step_x() * width + clock_wise.step_x() * offset,
                height,
                direction.step_z() * width + clock_wise.step_z() * offset,
            );
            if height < 0 && !level.block_state(pos).is_solid() {
                return false;
            }
            if height >= 0 && !can_portal_replace_block(level, pos) {
                return false;
            }
        }
    }
    true
}
