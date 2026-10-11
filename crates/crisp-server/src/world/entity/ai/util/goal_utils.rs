use crisp_core::BlockPos;
use crisp_data::fluid_tags;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::pathfinding_context::PathfindingContext;
use crate::world::level::pathfinder::walk_node_evaluator::get_path_type_static;

pub fn mob_restricted(level: &ServerLevel, mob: EntityId, horizontal_dist: f64) -> bool {
    let m = level.mob(mob).expect("a mob");
    let position = m.entity.position();
    let distance = m.home_radius as f64 + horizontal_dist + 1.0;
    m.has_home() && m.home_position.dist_to_center_sqr(position.x, position.y, position.z) < distance * distance
}

pub fn is_outside_limits(level: &ServerLevel, pos: BlockPos) -> bool {
    level.chunks.is_outside_build_height(pos.y)
}

pub fn is_restricted(level: &ServerLevel, restrict: bool, mob: EntityId, pos: BlockPos) -> bool {
    restrict && !level.mob(mob).expect("a mob").is_within_home(pos)
}

pub fn is_not_stable(level: &ServerLevel, mob: EntityId, pos: BlockPos) -> bool {
    !level.mob(mob).expect("a mob").ai.navigation.is_stable_destination(level, pos)
}

pub fn is_water(level: &ServerLevel, pos: BlockPos) -> bool {
    PathNavigationRegion::new(level).get_fluid_state(pos).is(&fluid_tags::WATER)
}

pub fn has_malus(level: &ServerLevel, mob: EntityId, pos: BlockPos) -> bool {
    let context = PathfindingContext::new(PathNavigationRegion::new(level), mob);
    level.mob(mob).expect("a mob").get_pathfinding_malus(get_path_type_static(&context, pos.x, pos.y, pos.z)) != 0.0
}

pub fn is_solid(level: &ServerLevel, pos: BlockPos) -> bool {
    PathNavigationRegion::new(level).get_block_state(pos).is_solid()
}
