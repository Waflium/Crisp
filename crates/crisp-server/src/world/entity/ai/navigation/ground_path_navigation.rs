use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::fluid_tags;

use super::path_navigation::PathNavigation;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::chunk::LevelChunk;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path::Path;
use crate::world::level::pathfinder::path_finder::PathFinder;
use crate::world::level::pathfinder::walk_node_evaluator::WalkNodeEvaluator;

impl PathNavigation {
    pub fn create_path_finder(&mut self, max_visited_nodes: i32) -> PathFinder {
        PathFinder::new(WalkNodeEvaluator::default(), max_visited_nodes)
    }

    pub fn can_update_path(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        e.on_ground || e.is_in_liquid() || level.is_passenger(mob)
    }

    pub fn get_temp_mob_pos(&self, level: &ServerLevel, mob: EntityId) -> Vec3 {
        let e = level.entity(mob);
        Vec3::new(e.x(), self.get_surface_y(level, mob) as f64, e.z())
    }

    pub fn create_path(&mut self, level: &ServerLevel, mob: EntityId, mut pos: BlockPos, reach_range: i32) -> Option<Path> {
        let chunk = level.chunks.loaded_chunk(pos.chunk())?;
        if !self.can_path_to_targets_below_surface {
            pos = self.find_surface_position(level, chunk, pos);
        }
        self.path_navigation_create_path(level, mob, pos, reach_range)
    }

    fn find_surface_position(&self, level: &ServerLevel, chunk: &LevelChunk, mut pos: BlockPos) -> BlockPos {
        let (min_y, max_y) = (level.chunks.min_y(), level.chunks.max_y());
        if chunk.block_state(pos).is_air() {
            let mut column_pos = pos.below();
            while column_pos.y >= min_y && chunk.block_state(column_pos).is_air() {
                column_pos = column_pos.below();
            }
            if column_pos.y >= min_y {
                return column_pos.above();
            }
            column_pos.y = pos.y + 1;
            while column_pos.y <= max_y && chunk.block_state(column_pos).is_air() {
                column_pos = column_pos.above();
            }
            pos = column_pos;
        }
        if !chunk.block_state(pos).is_solid() {
            return pos;
        }
        let mut column_pos = pos.above();
        while column_pos.y <= max_y && chunk.block_state(column_pos).is_solid() {
            column_pos = column_pos.above();
        }
        column_pos
    }

    pub fn create_path_to_entity(&mut self, level: &ServerLevel, mob: EntityId, target: EntityId, reach_range: i32) -> Option<Path> {
        let pos = level.entity(target).block_position();
        self.create_path(level, mob, pos, reach_range)
    }

    fn get_surface_y(&self, level: &ServerLevel, mob: EntityId) -> i32 {
        let e = level.entity(mob);
        if e.fluid_interaction.is_in_fluid(&fluid_tags::ENTITY_FLOATABLE) && self.can_float() {
            let region = PathNavigationRegion::new(level);
            let mut surface = e.block_position().y;
            let mut state = region.get_block_state(BlockPos::containing(e.x(), surface as f64, e.z()));
            let mut steps = 0;
            while state.fluid_state().is(&fluid_tags::ENTITY_FLOATABLE) {
                surface += 1;
                state = region.get_block_state(BlockPos::containing(e.x(), surface as f64, e.z()));
                steps += 1;
                if steps > 16 {
                    return e.block_position().y;
                }
            }
            surface
        } else {
            mth::floor(e.y() + 0.5)
        }
    }

    pub fn trim_path(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.path_navigation_trim_path(level);
        if !self.avoid_sun {
            return;
        }
        let e = level.entity(mob);
        if level.can_see_sky(BlockPos::containing(e.x(), e.y() + 0.5, e.z())) {
            return;
        }
        let Some(path) = &mut self.path else { return };
        for i in 0..path.get_node_count() {
            let node = path.get_node(i);
            if level.can_see_sky(BlockPos::new(node.x, node.y, node.z)) {
                path.truncate_nodes(i);
                return;
            }
        }
    }

    pub fn can_navigate_ground(&self) -> bool {
        true
    }

    pub fn set_can_walk_over_fences(&mut self, can_walk_over_fences: bool) {
        self.path_finder.node_evaluator.node_evaluator.can_walk_over_fences = can_walk_over_fences;
    }
}
