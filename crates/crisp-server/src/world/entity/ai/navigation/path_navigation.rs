use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::block_tags;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path::Path;
use crate::world::level::pathfinder::path_finder::PathFinder;
use crate::world::level::pathfinder::path_type::PathType;

pub struct PathNavigation {
    pub path: Option<Path>,
    pub speed_modifier: f64,
    pub tick: i32,
    last_stuck_check: i32,
    last_stuck_check_pos: Vec3,
    timeout_cached_node: BlockPos,
    timeout_timer: i64,
    last_timeout_check: i64,
    timeout_limit: f64,
    max_distance_to_waypoint: f32,
    has_delayed_recomputation: bool,
    time_last_recompute: i64,
    target_pos: Option<BlockPos>,
    reach_range: i32,
    max_visited_nodes_multiplier: f32,
    pub path_finder: PathFinder,
    is_stuck: bool,
    required_path_length: f32,
    pub avoid_sun: bool,
    pub can_path_to_targets_below_surface: bool,
}

impl Default for PathNavigation {
    fn default() -> PathNavigation {
        PathNavigation {
            path: None,
            speed_modifier: 0.0,
            tick: 0,
            last_stuck_check: 0,
            last_stuck_check_pos: Vec3::ZERO,
            timeout_cached_node: BlockPos::ZERO,
            timeout_timer: 0,
            last_timeout_check: 0,
            timeout_limit: 0.0,
            max_distance_to_waypoint: 0.5,
            has_delayed_recomputation: false,
            time_last_recompute: 0,
            target_pos: None,
            reach_range: 0,
            max_visited_nodes_multiplier: 1.0,
            path_finder: PathFinder::default(),
            is_stuck: false,
            required_path_length: 16.0,
            avoid_sun: false,
            can_path_to_targets_below_surface: false,
        }
    }
}

impl PathNavigation {
    pub fn new(follow_range_base: f64) -> PathNavigation {
        let mut navigation = PathNavigation::default();
        navigation.path_finder = navigation.create_path_finder(mth::floor(follow_range_base * 16.0));
        navigation
    }

    pub fn update_pathfinder_max_visited_nodes(&mut self, level: &ServerLevel, mob: EntityId) {
        let max_visited_nodes = mth::floor_f(self.get_max_path_length(level, mob) * 16.0);
        self.path_finder.set_max_visited_nodes(max_visited_nodes);
    }

    pub fn set_required_path_length(&mut self, level: &ServerLevel, mob: EntityId, length: f32) {
        self.required_path_length = length;
        self.update_pathfinder_max_visited_nodes(level, mob);
    }

    fn get_max_path_length(&self, level: &ServerLevel, mob: EntityId) -> f32 {
        (level.attribute_value(mob, Attribute::FollowRange) as f32).max(self.required_path_length)
    }

    pub fn reset_max_visited_nodes_multiplier(&mut self) {
        self.max_visited_nodes_multiplier = 1.0;
    }

    pub fn set_max_visited_nodes_multiplier(&mut self, max_visited_nodes_multiplier: f32) {
        self.max_visited_nodes_multiplier = max_visited_nodes_multiplier;
    }

    pub fn get_target_pos(&self) -> Option<BlockPos> {
        self.target_pos
    }

    pub fn set_speed_modifier(&mut self, speed_modifier: f64) {
        self.speed_modifier = speed_modifier;
    }

    pub fn recompute_path(&mut self, level: &ServerLevel, mob: EntityId) {
        if level.game_time - self.time_last_recompute <= 20 || !self.can_update_path(level, mob) {
            self.has_delayed_recomputation = true;
        } else if let Some(target_pos) = self.target_pos {
            self.path = None;
            self.path = self.create_path(level, mob, target_pos, self.reach_range);
            self.time_last_recompute = level.game_time;
            self.has_delayed_recomputation = false;
        }
    }

    pub fn create_path_to(&mut self, level: &ServerLevel, mob: EntityId, x: f64, y: f64, z: f64, reach_range: i32) -> Option<Path> {
        self.create_path(level, mob, BlockPos::containing(x, y, z), reach_range)
    }

    pub fn path_navigation_create_path(&mut self, level: &ServerLevel, mob: EntityId, pos: BlockPos, reach_range: i32) -> Option<Path> {
        self.create_path_from_targets(level, mob, &[pos], 8, false, reach_range)
    }

    fn create_path_from_targets(&mut self, level: &ServerLevel, mob: EntityId, targets: &[BlockPos], radius_offset: i32, above: bool, reach_range: i32) -> Option<Path> {
        let max_path_length = self.get_max_path_length(level, mob);
        self.create_path_with_max_length(level, mob, targets, radius_offset, above, reach_range, max_path_length)
    }

    #[allow(clippy::too_many_arguments)]
    fn create_path_with_max_length(&mut self, level: &ServerLevel, mob: EntityId, targets: &[BlockPos], _radius_offset: i32, _above: bool, reach_range: i32, max_path_length: f32) -> Option<Path> {
        if targets.is_empty() {
            return None;
        }
        if level.entity(mob).y() < level.chunks.min_y() as f64 {
            return None;
        }
        if !self.can_update_path(level, mob) {
            return None;
        }
        if let Some(path) = &self.path
            && !path.is_done()
            && self.target_pos.is_some_and(|target_pos| targets.contains(&target_pos))
        {
            return Some(path.clone());
        }
        let path = self.path_finder.find_path(PathNavigationRegion::new(level), mob, targets, max_path_length, reach_range, self.max_visited_nodes_multiplier);
        if let Some(path) = &path {
            self.target_pos = Some(path.get_target());
            self.reach_range = reach_range;
            self.reset_stuck_timeout();
        }
        path
    }

    pub fn move_to(&mut self, level: &mut ServerLevel, mob: EntityId, x: f64, y: f64, z: f64, speed_modifier: f64) -> bool {
        let path = self.create_path_to(level, mob, x, y, z, 1);
        self.move_to_path(level, mob, path, speed_modifier)
    }

    pub fn move_to_entity(&mut self, level: &mut ServerLevel, mob: EntityId, target: EntityId, speed_modifier: f64) -> bool {
        let new_path = self.create_path_to_entity(level, mob, target, 1);
        new_path.is_some() && self.move_to_path(level, mob, new_path, speed_modifier)
    }

    pub fn move_to_path(&mut self, level: &mut ServerLevel, mob: EntityId, new_path: Option<Path>, speed_modifier: f64) -> bool {
        let Some(new_path) = new_path else {
            self.path = None;
            return false;
        };
        if !new_path.same_as(self.path.as_ref()) {
            self.path = Some(new_path);
        }
        if self.is_done() {
            return false;
        }
        self.trim_path(level, mob);
        if self.path.as_ref().expect("a path").get_node_count() == 0 {
            return false;
        }
        self.speed_modifier = speed_modifier;
        let mob_pos = self.get_temp_mob_pos(level, mob);
        self.last_stuck_check = self.tick;
        self.last_stuck_check_pos = mob_pos;
        true
    }

    pub fn get_path(&self) -> Option<&Path> {
        self.path.as_ref()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.tick += 1;
        if self.has_delayed_recomputation {
            self.recompute_path(level, mob);
        }
        if self.is_done() {
            return;
        }
        if self.can_update_path(level, mob) {
            self.follow_the_path(level, mob);
        } else if self.path.as_ref().is_some_and(|path| !path.is_done()) {
            let mob_pos = self.get_temp_mob_pos(level, mob);
            let path = self.path.as_mut().expect("a path");
            let e = level.entity(mob);
            let pos = path.get_next_entity_pos(e);
            if mob_pos.y > pos.y && !e.on_ground && mth::floor(mob_pos.x) == mth::floor(pos.x) && mth::floor(mob_pos.z) == mth::floor(pos.z) {
                path.advance();
            }
        }
        if !self.is_done() {
            let target = self.path.as_ref().expect("a path").get_next_entity_pos(level.entity(mob));
            let ground_y = self.get_ground_y(level, target);
            level.mob_mut(mob).expect("a mob").ai.move_control.set_wanted_position(target.x, ground_y, target.z, self.speed_modifier);
        }
    }

    fn get_ground_y(&self, level: &ServerLevel, target: Vec3) -> f64 {
        let block_pos = BlockPos::containing_vec(target);
        let region = PathNavigationRegion::new(level);
        if region.get_block_state(block_pos.below()).is_air() {
            target.y
        } else {
            crate::world::level::pathfinder::walk_node_evaluator::get_floor_level(&region, block_pos)
        }
    }

    fn follow_the_path(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let mob_pos = self.get_temp_mob_pos(level, mob);
        let e = level.entity(mob);
        let width = e.dimensions.width;
        self.max_distance_to_waypoint = if width > 0.75 { width / 2.0 } else { 0.75 - width / 2.0 };
        let path = self.path.as_ref().expect("a path");
        let current_node_pos = path.get_next_node_pos();
        let x_distance = (e.x() - (current_node_pos.x as f64 + 0.5)).abs();
        let y_distance = (e.y() - current_node_pos.y as f64).abs();
        let z_distance = (e.z() - (current_node_pos.z as f64 + 0.5)).abs();
        let is_close_enough_to_current_node = x_distance < self.max_distance_to_waypoint as f64
            && z_distance < self.max_distance_to_waypoint as f64
            && y_distance < self.get_max_vertical_distance_to_waypoint() as f64;
        if is_close_enough_to_current_node || self.can_cut_corner(path.get_next_node().r#type) && self.should_target_next_node_in_direction(level, mob, mob_pos) {
            self.path.as_mut().expect("a path").advance();
        }
        self.do_stuck_detection(level, mob, mob_pos);
    }

    fn should_target_next_node_in_direction(&self, level: &ServerLevel, mob: EntityId, mob_position: Vec3) -> bool {
        let path = self.path.as_ref().expect("a path");
        if path.get_next_node_index() as usize + 1 >= path.get_node_count() {
            return false;
        }
        let current_node = Vec3::at_bottom_center_of(path.get_next_node_pos());
        if !mob_position.closer_than(current_node, 2.0) {
            return false;
        }
        if self.can_move_directly(mob_position, path.get_next_entity_pos(level.entity(mob))) {
            return true;
        }
        let next_node = Vec3::at_bottom_center_of(path.get_node_pos(path.get_next_node_index() as usize + 1));
        let mob_to_current = current_node.subtract(mob_position);
        let mob_to_next = next_node.subtract(mob_position);
        let mob_to_current_sqr = mob_to_current.length_sqr();
        let mob_to_next_sqr = mob_to_next.length_sqr();
        let closer_to_next_than_current = mob_to_next_sqr < mob_to_current_sqr;
        let within_current_block = mob_to_current_sqr < 0.5;
        if !closer_to_next_than_current && !within_current_block {
            return false;
        }
        let mob_direction = mob_to_current.normalize();
        let path_direction = mob_to_next.normalize();
        path_direction.dot(mob_direction) < 0.0
    }

    fn do_stuck_detection(&mut self, level: &ServerLevel, mob: EntityId, mob_pos: Vec3) {
        let speed = level.living(mob).speed;
        if self.tick - self.last_stuck_check > 100 {
            let effective_speed = if speed >= 1.0 { speed } else { speed * speed };
            let threshold_distance = effective_speed * 100.0 * 0.25;
            if mob_pos.distance_to_sqr(self.last_stuck_check_pos) < (threshold_distance * threshold_distance) as f64 {
                self.is_stuck = true;
                self.stop();
            } else {
                self.is_stuck = false;
            }
            self.last_stuck_check = self.tick;
            self.last_stuck_check_pos = mob_pos;
        }
        if let Some(path) = &self.path
            && !path.is_done()
        {
            let pos = path.get_next_node_pos();
            let time = level.game_time;
            if pos == self.timeout_cached_node {
                self.timeout_timer += time - self.last_timeout_check;
            } else {
                self.timeout_cached_node = pos;
                let dist_to_node = mob_pos.distance_to(Vec3::at_bottom_center_of(self.timeout_cached_node));
                self.timeout_limit = if speed > 0.0 { dist_to_node / speed as f64 * 20.0 } else { 0.0 };
            }
            if self.timeout_limit > 0.0 && self.timeout_timer as f64 > self.timeout_limit * 3.0 {
                self.timeout_path();
            }
            self.last_timeout_check = time;
        }
    }

    fn timeout_path(&mut self) {
        self.reset_stuck_timeout();
        self.stop();
    }

    fn reset_stuck_timeout(&mut self) {
        self.timeout_cached_node = BlockPos::ZERO;
        self.timeout_timer = 0;
        self.timeout_limit = 0.0;
        self.is_stuck = false;
    }

    pub fn is_done(&self) -> bool {
        self.path.as_ref().is_none_or(Path::is_done)
    }

    pub fn is_in_progress(&self) -> bool {
        !self.is_done()
    }

    pub fn stop(&mut self) {
        self.path = None;
    }

    pub fn path_navigation_trim_path(&mut self, level: &ServerLevel) {
        let Some(path) = &mut self.path else { return };
        let region = PathNavigationRegion::new(level);
        for i in 0..path.get_node_count() {
            let node = *path.get_node(i);
            let next_node = (i + 1 < path.get_node_count()).then(|| *path.get_node(i + 1));
            let state = region.get_block_state(BlockPos::new(node.x, node.y, node.z));
            if state.is(&block_tags::CAULDRONS) {
                path.replace_node(i, node.clone_and_move(node.x, node.y + 1, node.z));
                if let Some(next_node) = next_node
                    && node.y >= next_node.y
                {
                    path.replace_node(i + 1, node.clone_and_move(next_node.x, node.y + 1, next_node.z));
                }
            }
        }
    }

    fn can_move_directly(&self, _start_pos: Vec3, _stop_pos: Vec3) -> bool {
        false
    }

    pub fn can_cut_corner(&self, path_type: PathType) -> bool {
        path_type != PathType::FireInNeighbor && path_type != PathType::DamagingInNeighbor && path_type != PathType::WalkableDoor
    }

    pub fn is_stable_destination(&self, level: &ServerLevel, pos: BlockPos) -> bool {
        PathNavigationRegion::new(level).get_block_state(pos.below()).is_solid_render()
    }

    pub fn set_can_float(&mut self, can_float: bool) {
        self.path_finder.node_evaluator.node_evaluator.can_float = can_float;
    }

    pub fn can_float(&self) -> bool {
        self.path_finder.node_evaluator.node_evaluator.can_float
    }

    pub fn should_recompute_path(&self, level: &ServerLevel, mob: EntityId, pos: BlockPos) -> bool {
        if self.has_delayed_recomputation {
            return false;
        }
        let Some(path) = &self.path else { return false };
        if path.is_done() || path.get_node_count() == 0 {
            return false;
        }
        let target = path.get_end_node().expect("an end node");
        let e = level.entity(mob);
        let middle_pos = Vec3::new((target.x as f64 + e.x()) / 2.0, (target.y as f64 + e.y()) / 2.0, (target.z as f64 + e.z()) / 2.0);
        let distance = (path.get_node_count() as i32 - path.get_next_node_index()) as f64;
        pos.dist_to_center_sqr(middle_pos.x, middle_pos.y, middle_pos.z) < distance * distance
    }

    pub fn get_max_distance_to_waypoint(&self) -> f32 {
        self.max_distance_to_waypoint
    }

    pub fn get_max_vertical_distance_to_waypoint(&self) -> f32 {
        1.0
    }

    pub fn is_stuck(&self) -> bool {
        self.is_stuck
    }

    pub fn set_can_open_doors(&mut self, can_open_doors: bool) {
        self.path_finder.node_evaluator.node_evaluator.can_open_doors = can_open_doors;
    }
}
