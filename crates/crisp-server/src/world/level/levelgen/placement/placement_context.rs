use crisp_core::BlockPos;
use crisp_data::BlockState;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::material::WorldGenerationContext;

pub struct PlacementContext<'a, 'b> {
    pub level: &'a mut WorldGenRegion<'b>,
    pub generation: WorldGenerationContext,
    top_feature: Option<u32>,
}

impl<'a, 'b> PlacementContext<'a, 'b> {
    pub fn new(level: &'a mut WorldGenRegion<'b>, generation: WorldGenerationContext, top_feature: Option<u32>) -> PlacementContext<'a, 'b> {
        PlacementContext { level, generation, top_feature }
    }

    pub fn get_height(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32 {
        self.level.get_height(kind, x, z)
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        self.level.get_block_state(pos)
    }

    pub fn get_min_y(&self) -> i32 {
        self.level.get_min_y()
    }

    pub fn top_feature(&self) -> Option<u32> {
        self.top_feature
    }

    pub fn biome_has_top_feature(&self, origin: BlockPos) -> bool {
        let feature = self.top_feature.expect("Tried to biome check an unregistered feature, or a feature that should not restrict the biome");
        let biome = self.level.get_biome(origin);
        self.level.generator().get_biome_generation_settings(biome).has_feature(feature)
    }
}
