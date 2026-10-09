use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crisp_core::ChunkPos;
use crisp_data::{BlockState, DimensionType, block_tags, blocks};

use super::aquifer::{Aquifer, AquiferSamplers, FluidPicker, NoiseBasedAquifer};
use super::carver::BIOME_CARVERS;
use super::density_function::{Df, DensityVolume, Sampler, SamplerContext};
use super::material::WorldGenerationContext;
use super::worldgen_random::WorldgenRandom;
use super::{NoiseGeneratorSettings, RandomState, WORLDGEN, quart_pos};
use crate::world::level::biome::climate::{self, Parameter};
use crate::world::level::biome::{self, MultiNoiseBiomeSource};
use crate::world::level::chunk::carving_mask::CarvingMask;
use crate::world::level::chunk::proto_chunk::{ChunkStatus, ProtoChunk};
use crate::world::level::chunk::{Biome, HeightmapType};

const MAX_CACHED_BIOMES: usize = 16384;
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
            settings,
            biome_source,
            state,
            samplers,
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
        let region: Vec<Arc<ChunkBiomes>> =
            (0..9).map(|i| self.chunk_biomes(ChunkPos::new(pos.x + i % 3 - 1, pos.z + i / 3 - 1), dimension)).collect();
        self.build_terrain(pos, dimension, std::array::from_fn(|i| &*region[i]))
    }

    pub fn build_terrain(&self, pos: ChunkPos, dimension: &DimensionType, biomes: [&ChunkBiomes; 9]) -> ProtoChunk {
        let mut chunk = ProtoChunk::new(pos, dimension);
        let center = biomes[4];
        for (i, section) in chunk.sections.iter_mut().enumerate() {
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        section.biomes.set(x, y, z, center.get_local(x, i * 4 + y, z));
                    }
                }
            }
        }
        chunk.status = ChunkStatus::Biomes;
        let noise = self.settings.noise.clamp_to(dimension.min_y, dimension.height as i32);
        if noise.height <= 0 {
            return chunk;
        }
        let volume = DensityVolume::new(16, noise.height, 16, pos.min_block_x(), noise.min_y, pos.min_block_z());
        let mut ctx = SamplerContext::cached();
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
