use std::sync::Arc;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, ChunkPos};
use crisp_data::DimensionType;

use super::chunk_generator_structure_state::ChunkGeneratorStructureState;
use super::{Biome, ChunkGenerator, ChunkStructures, HeightmapType};
use crate::core::holder_set::HolderSet;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::biome::biome_generation_settings::{self, BiomeGenerationSettings};
use crate::world::level::biome::feature_sorter::StepFeatureData;
use crate::world::level::levelgen::density_function::SamplerContext;
use crate::world::level::levelgen::placement::feature_placer::FeaturePlacer;
use crate::world::level::levelgen::placement::placed_feature;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_set::StructureSelectionEntry;
use crate::world::level::levelgen::structure::structure_start::StructureStart;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;
use crate::world::level::noise_column::NoiseColumn;

const DECORATION_STEPS: usize = 11;

impl ChunkGenerator {
    pub fn possible_biomes(&self) -> Vec<Biome> {
        match self {
            ChunkGenerator::Flat(flat) => vec![flat.biome],
            ChunkGenerator::Noise(noise) => noise.biome_source.possible_biomes(),
        }
    }

    fn is_possible_biome(&self, biome: Biome) -> bool {
        match self {
            ChunkGenerator::Flat(flat) => biome == flat.biome,
            ChunkGenerator::Noise(noise) => noise.features_per_step().1[biome.0 as usize],
        }
    }

    pub fn get_noise_biome(&self, ctx: &mut SamplerContext, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        match self {
            ChunkGenerator::Flat(flat) => flat.biome,
            ChunkGenerator::Noise(noise) => noise.get_noise_biome(ctx, quart_x, quart_y, quart_z),
        }
    }

    pub fn find_biome_horizontal(&self, origin_x: i32, origin_y: i32, origin_z: i32, search_radius: i32, allowed: &HolderSet, random: &mut impl RandomSource) -> Option<(i32, i32, i32)> {
        match self {
            ChunkGenerator::Flat(flat) => allowed.contains(flat.biome.0 as i32).then(|| {
                let x = origin_x - search_radius + random.next_int_bounded(search_radius * 2 + 1);
                let z = origin_z - search_radius + random.next_int_bounded(search_radius * 2 + 1);
                (x, origin_y, z)
            }),
            ChunkGenerator::Noise(noise) => noise.find_biome_horizontal(origin_x, origin_y, origin_z, search_radius, allowed, random),
        }
    }

    pub fn get_base_height(&self, x: i32, z: i32, kind: HeightmapType, min_y: i32, height: i32) -> i32 {
        match self {
            ChunkGenerator::Flat(flat) => flat.get_base_height(kind, min_y, height),
            ChunkGenerator::Noise(noise) => noise.get_base_height(x, z, kind, min_y, height),
        }
    }

    pub fn get_first_occupied_height(&self, x: i32, z: i32, kind: HeightmapType, min_y: i32, height: i32) -> i32 {
        self.get_base_height(x, z, kind, min_y, height) - 1
    }

    pub fn get_base_column(&self, x: i32, z: i32, min_y: i32, height: i32) -> NoiseColumn {
        match self {
            ChunkGenerator::Flat(flat) => flat.get_base_column(min_y, height),
            ChunkGenerator::Noise(noise) => noise.get_base_column(x, z, min_y, height),
        }
    }

    pub fn min_y(&self) -> i32 {
        match self {
            ChunkGenerator::Flat(_) => 0,
            ChunkGenerator::Noise(noise) => noise.min_y(),
        }
    }

    pub fn gen_depth(&self) -> i32 {
        match self {
            ChunkGenerator::Flat(_) => 384,
            ChunkGenerator::Noise(noise) => noise.gen_depth(),
        }
    }

    pub fn get_biome_generation_settings(&self, biome: Biome) -> &BiomeGenerationSettings {
        match self {
            ChunkGenerator::Flat(flat) => flat.adjust_generation_settings(biome),
            ChunkGenerator::Noise(_) => biome_generation_settings::get(biome),
        }
    }

    fn features_per_step(&self) -> &[StepFeatureData] {
        match self {
            ChunkGenerator::Flat(flat) => flat.features_per_step(),
            ChunkGenerator::Noise(noise) => &noise.features_per_step().0,
        }
    }

    pub fn apply_biome_decoration(&self, level: &mut WorldGenRegion, structure_starts: &[Vec<Arc<StructureStart>>]) {
        let center_pos = level.get_center();
        let origin = BlockPos::new(center_pos.min_block_x(), level.get_min_y(), center_pos.min_block_z());
        let feature_list = self.features_per_step();
        let mut random = WorldgenRandom::xoroshiro(0);
        let decoration_seed = random.set_decoration_seed(level.get_seed(), origin.x, origin.z);
        let mut possible_biomes: Vec<Biome> = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let chunk = level.get_chunk(center_pos.x + dx, center_pos.z + dz);
                for section in &chunk.sections {
                    for x in 0..4 {
                        for y in 0..4 {
                            for z in 0..4 {
                                let biome = section.biomes.get(x, y, z);
                                if self.is_possible_biome(biome) && !possible_biomes.contains(&biome) {
                                    possible_biomes.push(biome);
                                }
                            }
                        }
                    }
                }
            }
        }
        let feature_step_count = feature_list.len();
        let mut placer = FeaturePlacer::default();
        let generation_steps = DECORATION_STEPS.max(feature_step_count);
        let mut possible_features_this_step: Vec<usize> = Vec::new();
        let writable_area = get_writable_area(center_pos, level.get_min_y(), level.get_max_y());
        for step_index in 0..generation_steps {
            if !structure_starts.is_empty() {
                for (index, structure) in StructureHolder::all().filter(|structure| structure.value().step() == step_index).enumerate() {
                    random.set_feature_seed(decoration_seed, index as i32, step_index as i32);
                    for start in &structure_starts[structure.0 as usize] {
                        start.place_in_chunk(level, &mut random, &writable_area, center_pos);
                    }
                }
            }
            if step_index >= feature_step_count {
                continue;
            }
            let step_feature_data = &feature_list[step_index];
            possible_features_this_step.clear();
            for &biome in &possible_biomes {
                let features_in_biome = self.get_biome_generation_settings(biome).features();
                if step_index < features_in_biome.len() {
                    for &feature in &features_in_biome[step_index] {
                        let index = step_feature_data.index_of(feature);
                        if !possible_features_this_step.contains(&index) {
                            possible_features_this_step.push(index);
                        }
                    }
                }
            }
            possible_features_this_step.sort_unstable();
            for &global_index_of_feature in &possible_features_this_step {
                let feature = step_feature_data.features[global_index_of_feature];
                random.set_feature_seed(decoration_seed, global_index_of_feature as i32, step_index as i32);
                placer.place_with_biome_check(level, placed_feature::get(feature), feature, &mut random, origin);
            }
        }
    }

    pub fn create_structures(&self, state: &ChunkGeneratorStructureState, source_chunk_pos: ChunkPos, dimension: &DimensionType, structures: &mut ChunkStructures) {
        state.ensure_structures_generated(self);
        for &set in state.possible_structure_sets() {
            let structure_set = set.value();
            let entries = &structure_set.structures;
            if entries.iter().any(|entry| structures.get_start_for_structure(entry.structure).is_some_and(|start| start.is_valid())) {
                continue;
            }
            if !structure_set.placement.is_structure_chunk(set, state, source_chunk_pos.x, source_chunk_pos.z) {
                continue;
            }
            if entries.len() == 1 {
                self.try_generate_structure(&entries[0], structures, state.get_level_seed(), source_chunk_pos, dimension);
                continue;
            }
            let mut options: Vec<&StructureSelectionEntry> = entries.iter().collect();
            let mut random = WorldgenRandom::legacy(0);
            random.set_large_feature_seed(state.get_level_seed(), source_chunk_pos.x, source_chunk_pos.z);
            let mut total: i32 = options.iter().map(|option| option.weight).sum();
            while !options.is_empty() {
                let mut choice = random.next_int_bounded(total);
                let mut index = 0;
                for option in &options {
                    choice -= option.weight;
                    if choice < 0 {
                        break;
                    }
                    index += 1;
                }
                let selected = options[index];
                if self.try_generate_structure(selected, structures, state.get_level_seed(), source_chunk_pos, dimension) {
                    break;
                }
                options.remove(index);
                total -= selected.weight;
            }
        }
    }

    fn try_generate_structure(&self, selected: &StructureSelectionEntry, structures: &mut ChunkStructures, seed: i64, source_chunk_pos: ChunkPos, dimension: &DimensionType) -> bool {
        let structure = selected.structure.value();
        let references = structures.get_start_for_structure(selected.structure).map_or(0, |start| start.get_references());
        match structure.generate(selected.structure, self, seed, source_chunk_pos, references, dimension.min_y, dimension.height as i32, structure.biomes()) {
            Some(start) => {
                structures.set_start_for_structure(selected.structure, Arc::new(start));
                true
            }
            None => false,
        }
    }
}

fn get_writable_area(chunk_pos: ChunkPos, min_y: i32, max_y: i32) -> BoundingBox {
    let (target_block_x, target_block_z) = (chunk_pos.min_block_x(), chunk_pos.min_block_z());
    BoundingBox::new(target_block_x, min_y + 1, target_block_z, target_block_x + 15, max_y, target_block_z + 15)
}
