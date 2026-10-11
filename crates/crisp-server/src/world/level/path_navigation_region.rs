use crisp_core::{AABB, BlockPos};
use crisp_data::{BlockState, FluidState, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct PathNavigationRegion<'a> {
    pub level: &'a ServerLevel,
}

impl<'a> PathNavigationRegion<'a> {
    pub fn new(level: &'a ServerLevel) -> PathNavigationRegion<'a> {
        PathNavigationRegion { level }
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        if self.level.chunks.is_outside_build_height(pos.y) {
            return blocks::AIR.default_state();
        }
        self.level.chunks.loaded_chunk(pos.chunk()).map_or(blocks::VOID_AIR.default_state(), |chunk| chunk.block_state(pos))
    }

    pub fn get_fluid_state(&self, pos: BlockPos) -> FluidState {
        self.get_block_state(pos).fluid_state()
    }

    pub fn no_collision(&self, entity: EntityId, aabb: &AABB) -> bool {
        self.level.no_collision_for(entity, aabb)
    }

    pub fn get_min_y(&self) -> i32 {
        self.level.chunks.min_y()
    }
}
