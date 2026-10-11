use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, ChunkPos};
use crisp_data::{BlockState, DimensionType, block_tags, blocks};

use super::aquifer::{Aquifer, AquiferSamplers, FluidPicker, NoiseBasedAquifer};
use super::carver::BIOME_CARVERS;
use super::density_function::{Df, DensityVolume, Sampler, SamplerContext};
use super::material::WorldGenerationContext;
use super::placement::feature_placer::FeaturePlacer;
use super::placement::placed_feature;
use super::worldgen_random::WorldgenRandom;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::biome::biome_generation_settings;
use crate::world::level::biome::feature_sorter::{self, StepFeatureData};
use super::{NoiseGeneratorSettings, RandomState, WORLDGEN, quart_pos};
use crate::world::level::biome::climate::{self, Parameter};
use crate::world::level::biome::{self, MultiNoiseBiomeSource};
use super::beardifier::Beardifier;
use super::structure::BoundingBox;
use super::structure::structure::StructureHolder;
use super::structure::structure_set::StructureSelectionEntry;
use super::structure::structure_start::StructureStart;
use crate::world::level::chunk::ChunkStructures;
use crate::world::level::chunk::carving_mask::CarvingMask;
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::chunk::proto_chunk::{ChunkStatus, ProtoChunk};
use crate::world::level::chunk::{Biome, HeightmapType, LevelChunkSection};
use crate::world::level::noise_column::NoiseColumn;

const MAX_CACHED_BIOMES: usize = 16384;
const DECORATION_STEPS: usize = 11;
const CARVER_RANGE: i32 = 8;
const PROTECTED_BLOCKS_ON_TOP: i32 = 7;

pub struct ChunkBiomes {
    min_quart_y: i32,
    quarts: Vec<Biome>,
}

impl ChunkBiomes {
    pub fn get(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        let max = self.min_quart_y + (self.quarts.len() / 16) as i32 - 1;
        let y = quart_y.clamp(self.min_quart_y, max) - self.min_quart_y;
        self.quarts[((y * 4 + (quart_x & 3)) * 4 + (quart_z & 3)) as usize]
    }

    fn get_local(&self, x: usize, y: usize, z: usize) -> Biome {
        self.quarts[(y * 4 + x) * 4 + z]
    }
}

struct Samplers {
    final_density: Arc<Sampler>,
    climate: [Arc<Sampler>; 6],
    aquifer: Option<AquiferSamplers>,
}

pub struct NoiseBasedChunkGenerator {
    pub settings: Arc<NoiseGeneratorSettings>,
    pub biome_source: MultiNoiseBiomeSource,
    pub state: RandomState,
    picker: FluidPicker,
    samplers: Samplers,
    zoom_seed: i64,
    biome_cache: Mutex<HashMap<ChunkPos, Arc<ChunkBiomes>>>,
    carver_biomes: Mutex<HashMap<ChunkPos, Biome>>,
    features_per_step: OnceLock<(Vec<StepFeatureData>, Vec<bool>)>,
}

impl NoiseBasedChunkGenerator {
    pub fn overworld(seed: i64) -> NoiseBasedChunkGenerator {
        let settings = WORLDGEN.noise_settings["minecraft:overworld"].clone();
        let biome_source = MultiNoiseBiomeSource::from_preset("minecraft:overworld").expect("overworld biome source");
        NoiseBasedChunkGenerator::new(settings, biome_source, seed)
    }

    pub fn new(settings: Arc<NoiseGeneratorSettings>, biome_source: MultiNoiseBiomeSource, seed: i64) -> NoiseBasedChunkGenerator {
        let state = RandomState::new(&settings, seed);
        let router = &settings.noise_router;
        let samplers = Samplers {
            final_density: state.sampler(&router.final_density),
            climate: [&router.temperature, &router.vegetation, &router.continents, &router.erosion, &router.depth, &router.ridges].map(|f| state.sampler(f)),
            aquifer: settings.aquifers.as_ref().map(|config| AquiferSamplers::new(&state, config)),
        };
        NoiseBasedChunkGenerator {
            picker: FluidPicker::new(settings.sea_level, settings.default_fluid),
            zoom_seed: biome::obfuscate_seed(seed),
            biome_cache: Mutex::new(HashMap::new()),
            carver_biomes: Mutex::new(HashMap::new()),
            features_per_step: OnceLock::new(),
            settings,
            biome_source,
            state,
            samplers,
        }
    }

    fn features_per_step(&self) -> &(Vec<StepFeatureData>, Vec<bool>) {
        self.features_per_step.get_or_init(|| {
            let possible_biomes = self.biome_source.possible_biomes();
            let mut possible = vec![false; crisp_data::SyncedRegistry::get("minecraft:worldgen/biome").entries.len()];
            for biome in &possible_biomes {
                possible[biome.0 as usize] = true;
            }
            (feature_sorter::build_features_per_step(&possible_biomes), possible)
        })
    }

    pub fn apply_biome_decoration(&self, level: &mut WorldGenRegion, structure_starts: &[Vec<Arc<StructureStart>>]) {
        let center_pos = level.get_center();
        let origin = BlockPos::new(center_pos.min_block_x(), level.get_min_y(), center_pos.min_block_z());
        let (feature_list, possible) = self.features_per_step();
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
                                if possible[biome.0 as usize] && !possible_biomes.contains(&biome) {
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
                let features_in_biome = biome_generation_settings::get(biome).features();
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

    pub fn seed(&self) -> i64 {
        self.state.seed
    }

    pub fn sea_level(&self) -> i32 {
        self.settings.sea_level
    }

    pub fn min_y(&self) -> i32 {
        self.settings.noise.min_y
    }

    pub fn gen_depth(&self) -> i32 {
        self.settings.noise.height
    }

    pub fn create_biomes(&self, pos: ChunkPos, dimension: &DimensionType) -> ChunkBiomes {
        let mut ctx = SamplerContext::cached();
        let min_quart_x = quart_pos::from_block(pos.min_block_x());
        let min_quart_y = quart_pos::from_block(dimension.min_y);
        let min_quart_z = quart_pos::from_block(pos.min_block_z());
        let size_y = quart_pos::from_block(dimension.height as i32);
        let volume = DensityVolume::stepped(
            4,
            size_y,
            4,
            quart_pos::to_block(min_quart_x),
            quart_pos::to_block(min_quart_y),
            quart_pos::to_block(min_quart_z),
            4,
            4,
            4,
        );
        let buffers: Vec<Vec<f32>> = self
            .samplers
            .climate
            .iter()
            .map(|sampler| {
                let mut buffer = vec![0.0; volume.size()];
                sampler.sample_volume(&mut ctx, &mut buffer, &volume);
                buffer
            })
            .collect();
        let sections = dimension.height as usize / 16;
        let mut quarts = vec![Biome(0); sections * 64];
        for section in 0..sections as i32 {
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        let quart_y = section * 4 + y;
                        let index = volume.index(x, quart_y, z);
                        let target = climate::target(
                            buffers[0][index],
                            buffers[1][index],
                            buffers[2][index],
                            buffers[3][index],
                            buffers[4][index],
                            buffers[5][index],
                        );
                        quarts[((quart_y * 4 + x) * 4 + z) as usize] = self.biome_source.get_noise_biome(&target);
                    }
                }
            }
        }
        ChunkBiomes { min_quart_y, quarts }
    }

    pub fn chunk_biomes(&self, pos: ChunkPos, dimension: &DimensionType) -> Arc<ChunkBiomes> {
        if let Some(biomes) = self.biome_cache.lock().unwrap().get(&pos) {
            return biomes.clone();
        }
        let biomes = Arc::new(self.create_biomes(pos, dimension));
        let mut cache = self.biome_cache.lock().unwrap();
        if cache.len() >= MAX_CACHED_BIOMES {
            cache.clear();
        }
        cache.entry(pos).or_insert(biomes).clone()
    }

    pub fn generate(&self, pos: ChunkPos, dimension: &DimensionType) -> ProtoChunk {
        self.generate_with_beardifier(pos, dimension, None)
    }

    pub fn generate_with_beardifier(&self, pos: ChunkPos, dimension: &DimensionType, beardifier: Option<Arc<Beardifier>>) -> ProtoChunk {
        let region: Vec<Arc<ChunkBiomes>> =
            (0..9).map(|i| self.chunk_biomes(ChunkPos::new(pos.x + i % 3 - 1, pos.z + i / 3 - 1), dimension)).collect();
        self.build_terrain(pos, dimension, std::array::from_fn(|i| &*region[i]), beardifier)
    }

    pub fn fill_biomes_from_noise(sections: &mut [LevelChunkSection], biomes: &ChunkBiomes) {
        for (i, section) in sections.iter_mut().enumerate() {
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        section.biomes.set(x, y, z, biomes.get_local(x, i * 4 + y, z));
                    }
                }
            }
        }
    }

    pub fn build_terrain(&self, pos: ChunkPos, dimension: &DimensionType, biomes: [&ChunkBiomes; 9], beardifier: Option<Arc<Beardifier>>) -> ProtoChunk {
        let mut chunk = ProtoChunk::new(pos, dimension);
        NoiseBasedChunkGenerator::fill_biomes_from_noise(&mut chunk.sections, biomes[4]);
        chunk.status = ChunkStatus::Biomes;
        let noise = self.settings.noise.clamp_to(dimension.min_y, dimension.height as i32);
        if noise.height <= 0 {
            return chunk;
        }
        let volume = DensityVolume::new(16, noise.height, 16, pos.min_block_x(), noise.min_y, pos.min_block_z());
        let mut ctx = SamplerContext::cached();
        ctx.beardifier = beardifier;
        let mut aquifer = self.create_aquifer(&mut ctx, &volume);
        self.do_fill(&mut chunk, &mut ctx, &mut aquifer, &volume);

        let mut possible = vec![false; crisp_data::SyncedRegistry::get("minecraft:worldgen/biome").entries.len()];
        for chunk_biomes in biomes {
            for biome in &chunk_biomes.quarts {
                possible[biome.0 as usize] = true;
            }
        }
        let zoom_seed = self.zoom_seed;
        let (center_x, center_z) = (pos.x, pos.z);
        let mut biome_getter = |x: i32, y: i32, z: i32| {
            let (qx, qy, qz) = biome::zoomed_quart(zoom_seed, x, y, z);
            let (cx, cz) = ((qx >> 2) - center_x, (qz >> 2) - center_z);
            biomes[((cz + 1) * 3 + cx + 1) as usize].get(qx, qy, qz)
        };
        let generation = WorldGenerationContext {
            min_gen_y: dimension.min_y.max(self.min_y()),
            gen_depth: (dimension.height as i32).min(self.gen_depth()),
            sea_level: self.sea_level(),
        };
        self.state.material_system().build_surface(
            &self.state,
            &mut chunk,
            &mut ctx,
            volume,
            &mut biome_getter,
            generation,
            &self.settings.material_rule,
            Some(&possible),
        );
        self.generate_carvers(&mut chunk, &mut ctx, &mut aquifer, &mut biome_getter, generation);
        chunk.prime_heightmaps(&HeightmapType::LIVE);
        chunk.status = ChunkStatus::Terrain;
        chunk
    }

    fn generate_carvers(
        &self,
        chunk: &mut ProtoChunk,
        ctx: &mut SamplerContext,
        aquifer: &mut Aquifer,
        biome_getter: &mut dyn FnMut(i32, i32, i32) -> Biome,
        generation: WorldGenerationContext,
    ) {
        let pos = chunk.pos;
        let mut random = WorldgenRandom::legacy(0);
        let max_y = generation.min_gen_y + generation.gen_depth - 1 - PROTECTED_BLOCKS_ON_TOP;
        let mut mask = CarvingMask::new(generation.min_gen_y + 1, max_y);
        let mut climate_ctx = SamplerContext::uncached();
        for dx in -CARVER_RANGE..=CARVER_RANGE {
            for dz in -CARVER_RANGE..=CARVER_RANGE {
                let source = ChunkPos::new(pos.x + dx, pos.z + dz);
                let biome = self.carver_biome(&mut climate_ctx, source);
                for (index, carver) in BIOME_CARVERS[biome.0 as usize].iter().enumerate() {
                    random.set_large_feature_seed(self.seed().wrapping_add(index as i64), source.x, source.z);
                    if carver.is_start_chunk(&mut random) {
                        carver.carve(&generation, &mut random, pos, source, &mut mask);
                    }
                }
            }
        }
        if !mask.is_empty() {
            self.apply_carving_mask(chunk, &mask, ctx, aquifer, biome_getter, generation);
        }
    }

    fn carver_biome(&self, ctx: &mut SamplerContext, chunk: ChunkPos) -> Biome {
        if let Some(&biome) = self.carver_biomes.lock().unwrap().get(&chunk) {
            return biome;
        }
        let (x, z) = (chunk.min_block_x(), chunk.min_block_z());
        let [t, v, c, e, d, w] = &self.samplers.climate;
        let target = climate::target(
            t.sample_value(ctx, x, 0, z),
            v.sample_value(ctx, x, 0, z),
            c.sample_value(ctx, x, 0, z),
            e.sample_value(ctx, x, 0, z),
            d.sample_value(ctx, x, 0, z),
            w.sample_value(ctx, x, 0, z),
        );
        let biome = self.biome_source.get_noise_biome(&target);
        let mut cache = self.carver_biomes.lock().unwrap();
        if cache.len() >= MAX_CACHED_BIOMES {
            cache.clear();
        }
        cache.insert(chunk, biome);
        biome
    }

    fn apply_carving_mask(
        &self,
        chunk: &mut ProtoChunk,
        mask: &CarvingMask,
        ctx: &mut SamplerContext,
        aquifer: &mut Aquifer,
        biome_getter: &mut dyn FnMut(i32, i32, i32) -> Biome,
        generation: WorldGenerationContext,
    ) {
        let (min_x, min_z) = (chunk.pos.min_block_x(), chunk.pos.min_block_z());
        mask.visit(|x, z, bottom_y, top_y| {
            let mut has_grass = false;
            let (world_x, world_z) = (min_x + x, min_z + z);
            for world_y in (bottom_y..=top_y).rev() {
                let old = chunk.block_state(x, world_y, z);
                if old.is(&block_tags::UNCARVABLE) {
                    continue;
                }
                if old.is_block(blocks::GRASS_BLOCK) || old.is_block(blocks::MYCELIUM) {
                    has_grass = true;
                }
                let Some(state) = aquifer.compute_substance(ctx, world_x, world_y, world_z, 0.0) else {
                    continue;
                };
                chunk.set_block_state(x, world_y, z, state);
                if aquifer.should_schedule_fluid_update() && !state.fluid_state().is_empty() {
                    chunk.mark_pos_for_post_processing(world_x, world_y, world_z);
                }
                if has_grass && chunk.block_state(x, world_y - 1, z).is_block(blocks::DIRT) {
                    let under_fluid = !state.fluid_state().is_empty();
                    let top = self.state.material_system().top_material(
                        &self.settings.material_rule,
                        &self.state,
                        generation,
                        biome_getter,
                        chunk,
                        ctx,
                        (world_x, world_y - 1, world_z),
                        under_fluid,
                    );
                    if let Some(top) = top {
                        chunk.set_block_state(x, world_y - 1, z, top);
                        if !top.fluid_state().is_empty() {
                            chunk.mark_pos_for_post_processing(world_x, world_y - 1, world_z);
                        }
                    }
                }
            }
        });
    }

    fn do_fill(&self, chunk: &mut ProtoChunk, ctx: &mut SamplerContext, aquifer: &mut Aquifer, volume: &DensityVolume) {
        chunk.heightmap_mut(HeightmapType::OceanFloorWg);
        chunk.heightmap_mut(HeightmapType::WorldSurfaceWg);
        let mut densities = ctx.acquire(volume.size());
        self.samplers.final_density.sample_volume(ctx, &mut densities, volume);
        let air = blocks::AIR.default_state();
        let default_block = self.settings.default_block;
        for z in 0..volume.size_z {
            let block_z = volume.block_z(z);
            for x in 0..volume.size_x {
                let block_x = volume.block_x(x);
                for y in (0..volume.size_y).rev() {
                    let block_y = volume.block_y(y);
                    let density = densities[volume.index(x, y, z)];
                    let state = aquifer.compute_substance(ctx, block_x, block_y, block_z, density as f64).unwrap_or(default_block);
                    if state != air {
                        self.place(chunk, x, block_y, z, state);
                        if aquifer.should_schedule_fluid_update() && !state.fluid_state().is_empty() {
                            chunk.mark_pos_for_post_processing(block_x, block_y, block_z);
                        }
                    }
                }
            }
        }
        ctx.release(densities);
    }

    fn place(&self, chunk: &mut ProtoChunk, x: i32, y: i32, z: i32, state: BlockState) {
        let index = chunk.section_index(y);
        chunk.sections[index].set_block_state(x as usize, (y & 15) as usize, z as usize, state);
        chunk.update_heightmap(HeightmapType::OceanFloorWg, x, y, z, state);
        chunk.update_heightmap(HeightmapType::WorldSurfaceWg, x, y, z, state);
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

    pub fn get_noise_biome(&self, ctx: &mut SamplerContext, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        let (block_x, block_y, block_z) = (quart_pos::to_block(quart_x), quart_pos::to_block(quart_y), quart_pos::to_block(quart_z));
        let [temperature, humidity, continentalness, erosion, depth, weirdness] =
            self.samplers.climate.each_ref().map(|sampler| sampler.sample_value(ctx, block_x, block_y, block_z));
        self.biome_source.get_noise_biome(&climate::target(temperature, humidity, continentalness, erosion, depth, weirdness))
    }

    pub fn get_base_height(&self, x: i32, z: i32, kind: HeightmapType, min_y: i32, height: i32) -> i32 {
        self.iterate_noise_column(min_y, height, x, z, |state| kind.is_opaque(state)).unwrap_or(min_y)
    }

    pub fn get_first_occupied_height(&self, x: i32, z: i32, kind: HeightmapType, min_y: i32, height: i32) -> i32 {
        self.get_base_height(x, z, kind, min_y, height) - 1
    }

    pub fn get_base_column(&self, x: i32, z: i32, min_y: i32, height: i32) -> NoiseColumn {
        let noise = self.settings.noise.clamp_to(min_y, height);
        let mut column = Vec::with_capacity(noise.height.max(0) as usize);
        self.iterate_noise_column(min_y, height, x, z, |state| {
            column.push(state);
            false
        });
        column.reverse();
        NoiseColumn::new(noise.min_y, column)
    }

    fn iterate_noise_column(&self, min_y: i32, height: i32, block_x: i32, block_z: i32, mut tester: impl FnMut(BlockState) -> bool) -> Option<i32> {
        let noise = self.settings.noise.clamp_to(min_y, height);
        if noise.height <= 0 {
            return None;
        }
        let volume = DensityVolume::new(1, noise.height, 1, block_x, noise.min_y, block_z);
        let mut ctx = SamplerContext::cached();
        let mut aquifer = self.create_aquifer(&mut ctx, &volume);
        let default_block = self.settings.default_block;
        let mut densities = ctx.acquire(volume.size());
        self.samplers.final_density.sample_volume(&mut ctx, &mut densities, &volume);
        let mut result = None;
        for y in (0..volume.size_y).rev() {
            let density = densities[volume.index(0, y, 0)];
            let block_y = volume.block_y(y);
            let state = aquifer.compute_substance(&mut ctx, block_x, block_y, block_z, density as f64).unwrap_or(default_block);
            if tester(state) {
                result = Some(block_y + 1);
                break;
            }
        }
        ctx.release(densities);
        result
    }

    pub fn origin(&self) -> ChunkPos {
        if self.settings.spawn_target.is_empty() {
            return ChunkPos::new(0, 0);
        }
        let mut ctx = SamplerContext::cached();
        let targets: Vec<Vec<(Arc<Sampler>, Parameter)>> = self
            .settings
            .spawn_target
            .iter()
            .map(|point| point.parameters.iter().map(|(f, p)| (self.state.sampler(&inline(f)), *p)).collect())
            .collect();
        let (x, z) = find_spawn_position(&mut ctx, &targets);
        ChunkPos::new(x >> 4, z >> 4)
    }

    fn create_aquifer(&self, ctx: &mut SamplerContext, volume: &DensityVolume) -> Aquifer {
        match &self.samplers.aquifer {
            Some(samplers) => {
                let random = self.state.random_factory("minecraft:aquifer");
                Aquifer::NoiseBased(Box::new(NoiseBasedAquifer::new(ctx, samplers.clone(), random, volume, self.picker)))
            }
            None => Aquifer::Disabled(self.picker),
        }
    }
}

fn get_writable_area(chunk_pos: ChunkPos, min_y: i32, max_y: i32) -> BoundingBox {
    let (target_block_x, target_block_z) = (chunk_pos.min_block_x(), chunk_pos.min_block_z());
    BoundingBox::new(target_block_x, min_y + 1, target_block_z, target_block_x + 15, max_y, target_block_z + 15)
}

fn inline(function: &Df) -> Df {
    match &**function {
        super::density_function::DensityFunction::Reference(r) => r.value.clone(),
        _ => function.clone(),
    }
}

fn find_spawn_position(ctx: &mut SamplerContext, targets: &[Vec<(Arc<Sampler>, Parameter)>]) -> (i32, i32) {
    let fitness = |ctx: &mut SamplerContext, x: i32, z: i32| -> i64 {
        let (qx, qz) = (quart_pos::to_block(quart_pos::from_block(x)), quart_pos::to_block(quart_pos::from_block(z)));
        let mut min_fitness = i64::MAX;
        for point in targets {
            let mut fitness = 0i64;
            for (sampler, parameter) in point {
                let value = climate::quantize_coord(sampler.sample_value(ctx, qx, 0, qz));
                let d = parameter.distance(value);
                fitness = fitness.wrapping_add(d.wrapping_mul(d));
            }
            min_fitness = min_fitness.min(fitness);
        }
        let bias = (x as i64) * (x as i64) + (z as i64) * (z as i64);
        min_fitness.wrapping_mul(2048 * 2048).wrapping_add(bias)
    };
    let mut best = ((0, 0), fitness(ctx, 0, 0));
    for (max_radius, increment) in [(2048.0f32, 512.0f32), (512.0, 32.0)] {
        let mut angle = 0.0f32;
        let mut radius = increment;
        let origin = best.0;
        while radius <= max_radius {
            let x = origin.0 + ((angle as f64).sin() * radius as f64) as i32;
            let z = origin.1 + ((angle as f64).cos() * radius as f64) as i32;
            let candidate = fitness(ctx, x, z);
            if candidate < best.1 {
                best = ((x, z), candidate);
            }
            angle += increment / radius;
            if angle as f64 > std::f64::consts::PI * 2.0 {
                angle = 0.0;
                radius += increment;
            }
        }
    }
    best.0
}
