use crisp_core::BlockPos;
use crisp_data::{BlockState, FluidState};

use super::LevelAccessor;
use super::chunk::{Biome, ChunkGenerator, HeightmapType};
use super::levelgen::material::WorldGenerationContext;

pub trait WorldGenLevel: LevelAccessor {
    fn get_seed(&self) -> i64;

    fn ensure_can_write(&self, pos: BlockPos) -> bool;

    fn generator(&self) -> &ChunkGenerator;

    fn get_difficulty(&self) -> crate::world::difficulty::Difficulty;

    fn generation(&self) -> WorldGenerationContext;

    fn get_block_state(&mut self, pos: BlockPos) -> BlockState;

    fn get_fluid_state(&mut self, pos: BlockPos) -> FluidState {
        self.get_block_state(pos).fluid_state()
    }

    fn get_height(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32;

    fn get_height_of_level(&self) -> i32;

    fn get_max_y(&self) -> i32 {
        self.get_min_y() + self.get_height_of_level() - 1
    }

    fn is_outside_build_height(&self, y: i32) -> bool {
        y < self.get_min_y() || y >= self.get_min_y() + self.get_height_of_level()
    }

    fn get_biome(&mut self, pos: BlockPos) -> Biome;

    fn set_block_and_update(&mut self, pos: BlockPos, state: BlockState) -> bool {
        self.set_block(pos, state, super::update_flags::UPDATE_ALL)
    }

    fn set_section_block_state(&mut self, pos: BlockPos, state: BlockState);

    fn destroy_block(&mut self, pos: BlockPos, drop_resources: bool) -> bool;

    fn add_fresh_entity(&mut self, entity: crate::world::entity::LevelEntity) -> bool;
}
