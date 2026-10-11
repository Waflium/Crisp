use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::path_type::PathType;
use crate::world::entity::EntityId;
use crate::world::level::path_navigation_region::PathNavigationRegion;

pub struct PathfindingContext<'a> {
    level: PathNavigationRegion<'a>,
    mob_position: BlockPos,
}

impl<'a> PathfindingContext<'a> {
    pub fn new(level: PathNavigationRegion<'a>, mob: EntityId) -> PathfindingContext<'a> {
        let mob_position = level.level.entity(mob).block_position();
        PathfindingContext { level, mob_position }
    }

    pub fn get_path_type_from_state(&self, x: i32, y: i32, z: i32) -> PathType {
        let pos = BlockPos::new(x, y, z);
        self.level.level.path_types_by_pos_cache.borrow_mut().get_or_compute(&self.level, pos)
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        self.level.get_block_state(pos)
    }

    pub fn level(&self) -> &PathNavigationRegion<'a> {
        &self.level
    }

    pub fn mob_position(&self) -> BlockPos {
        self.mob_position
    }
}
