use crisp_core::BlockPos;

use super::placed_feature::PlacedFeature;
use super::placement_context::PlacementContext;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;

#[derive(Default)]
pub struct FeaturePlacer {
    positions: Vec<BlockPos>,
    modifier_indices: Vec<usize>,
    modified_positions: Vec<BlockPos>,
}

impl FeaturePlacer {
    pub fn place(&mut self, level: &mut WorldGenRegion, placed_feature: &PlacedFeature, random: &mut WorldgenRandom, origin: BlockPos) -> bool {
        self.do_place(level, placed_feature, None, random, origin)
    }

    pub fn place_with_biome_check(&mut self, level: &mut WorldGenRegion, placed_feature: &PlacedFeature, index: u32, random: &mut WorldgenRandom, origin: BlockPos) -> bool {
        self.do_place(level, placed_feature, Some(index), random, origin)
    }

    fn do_place(&mut self, level: &mut WorldGenRegion, placed_feature: &PlacedFeature, top_feature: Option<u32>, random: &mut WorldgenRandom, origin: BlockPos) -> bool {
        let generation = level.generation();
        let mut context = PlacementContext::new(level, generation, top_feature);
        let feature = placed_feature.feature.value();
        let placement = &placed_feature.placement;
        if placement.is_empty() {
            return feature.place(context.level, random, origin);
        }
        let mut placed_any = false;
        self.positions.push(origin);
        self.modifier_indices.push(0);
        while let Some(pos) = self.positions.pop() {
            let modifier_index = self.modifier_indices.pop().expect("a modifier index per position");
            placement[modifier_index].modify(&mut context, random, pos, &mut self.modified_positions);
            let next_modifier_index = modifier_index + 1;
            if next_modifier_index < placement.len() {
                for &modified in self.modified_positions.iter().rev() {
                    self.positions.push(modified);
                    self.modifier_indices.push(next_modifier_index);
                }
            } else {
                for &next_pos in &self.modified_positions {
                    placed_any |= feature.place(context.level, random, next_pos);
                }
            }
            self.modified_positions.clear();
        }
        placed_any
    }
}

impl PlacedFeature {
    pub fn place(&self, level: &mut WorldGenRegion, random: &mut WorldgenRandom, origin: BlockPos) -> bool {
        FeaturePlacer::default().place(level, self, random, origin)
    }
}
