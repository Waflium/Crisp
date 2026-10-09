mod rules;

use std::sync::Arc;

use crisp_core::mth::{self, jmin};
use crisp_core::{BlockPos, RandomSource};
use crisp_data::{BlockState, blocks};

pub use rules::{MaterialParser, MaterialRule, Rule, RuleContext, VerticalAnchor, WorldGenerationContext};

use super::aquifer::WAY_BELOW_MIN_Y;
use super::density_function::{Df, DensityVolume, SamplerContext};
use super::synth::NoiseStack;
use super::{PositionalRandomFactory, RandomState};
use crate::world::level::chunk::Biome;
use crate::world::level::chunk::proto_chunk::ProtoChunk;
use crate::world::level::chunk::HeightmapType;

pub struct MaterialSystem {
    default_block: BlockState,
    pub sea_level: i32,
    pub preliminary_surface_function: Df,
    clay_bands: [BlockState; 192],
    clay_bands_offset: Arc<NoiseStack>,
    badlands_pillar: Arc<NoiseStack>,
    badlands_pillar_roof: Arc<NoiseStack>,
    badlands_surface: Arc<NoiseStack>,
    iceberg_pillar: Arc<NoiseStack>,
    iceberg_pillar_roof: Arc<NoiseStack>,
    iceberg_surface: Arc<NoiseStack>,
    noise_random: PositionalRandomFactory,
    surface: Arc<NoiseStack>,
    surface_secondary: Arc<NoiseStack>,
    eroded_badlands: Biome,
    frozen_ocean: Biome,
    deep_frozen_ocean: Biome,
}

fn make_bands(random: &mut impl RandomSource, bands: &mut [BlockState; 192], base_width: i32, state: BlockState) {
    let count = random.next_int_between_inclusive(6, 15);
    for _ in 0..count {
        let width = base_width + random.next_int_bounded(3);
        let start = random.next_int_bounded(bands.len() as i32);
        let mut p = 0;
        while start + p < bands.len() as i32 && p < width {
            bands[(start + p) as usize] = state;
            p += 1;
        }
    }
}

fn generate_bands(random: &mut impl RandomSource) -> [BlockState; 192] {
    let terracotta = blocks::TERRACOTTA.default_state();
    let mut bands = [terracotta; 192];
    let mut i = 0;
    while i < bands.len() {
        i += random.next_int_bounded(5) as usize + 1;
        if i < bands.len() {
            bands[i] = blocks::ORANGE_TERRACOTTA.default_state();
        }
        i += 1;
    }
    make_bands(random, &mut bands, 1, blocks::YELLOW_TERRACOTTA.default_state());
    make_bands(random, &mut bands, 2, blocks::BROWN_TERRACOTTA.default_state());
    make_bands(random, &mut bands, 1, blocks::RED_TERRACOTTA.default_state());
    let white_count = random.next_int_between_inclusive(9, 15);
    let mut placed = 0;
    let mut start = 0;
    while placed < white_count && start < bands.len() {
        bands[start] = blocks::WHITE_TERRACOTTA.default_state();
        if start > 1 && random.next_boolean() {
            bands[start - 1] = blocks::LIGHT_GRAY_TERRACOTTA.default_state();
        }
        if start + 1 < bands.len() && random.next_boolean() {
            bands[start + 1] = blocks::LIGHT_GRAY_TERRACOTTA.default_state();
        }
        placed += 1;
        start += random.next_int_bounded(16) as usize + 4;
    }
    bands
}

impl MaterialSystem {
    pub fn new(state: &RandomState, default_block: BlockState, sea_level: i32, preliminary_surface_function: Df, noise_random: PositionalRandomFactory) -> MaterialSystem {
        let biome = |name| Biome::by_name(name).expect("vanilla biome");
        MaterialSystem {
            default_block,
            sea_level,
            preliminary_surface_function,
            clay_bands_offset: state.noise("minecraft:clay_bands_offset"),
            clay_bands: generate_bands(&mut noise_random.from_hash_of("minecraft:clay_bands")),
            surface: state.noise("minecraft:surface"),
            surface_secondary: state.noise("minecraft:surface_secondary"),
            badlands_pillar: state.noise("minecraft:badlands_pillar"),
            badlands_pillar_roof: state.noise("minecraft:badlands_pillar_roof"),
            badlands_surface: state.noise("minecraft:badlands_surface"),
            iceberg_pillar: state.noise("minecraft:iceberg_pillar"),
            iceberg_pillar_roof: state.noise("minecraft:iceberg_pillar_roof"),
            iceberg_surface: state.noise("minecraft:iceberg_surface"),
            noise_random,
            eroded_badlands: biome("minecraft:eroded_badlands"),
            frozen_ocean: biome("minecraft:frozen_ocean"),
            deep_frozen_ocean: biome("minecraft:deep_frozen_ocean"),
        }
    }

    pub fn surface_depth(&self, x: i32, z: i32) -> i32 {
        let noise = self.surface.get(x as f64, 0.0, z as f64) as f64;
        (noise * 2.75 + 3.0 + self.noise_random.at(x, 0, z).next_double() * 0.25) as i32
    }

    pub fn surface_secondary(&self, x: i32, z: i32) -> f64 {
        self.surface_secondary.get(x as f64, 0.0, z as f64) as f64
    }

    pub fn band(&self, x: i32, y: i32, z: i32) -> BlockState {
        let offset = mth::java_round_f32(self.clay_bands_offset.get(x as f64, 0.0, z as f64) * 4.0);
        let len = self.clay_bands.len() as i32;
        self.clay_bands[((y + offset + len) % len) as usize]
    }

    fn is_stone(state: BlockState) -> bool {
        !state.is_air() && state.fluid_state().is_empty()
    }

    fn set_column(chunk: &mut ProtoChunk, x: i32, y: i32, z: i32, state: BlockState) {
        if y >= chunk.min_y && y <= chunk.max_y() {
            chunk.set_block_state(x, y, z, state);
            if !state.fluid_state().is_empty() {
                chunk.mark_pos_for_post_processing(x, y, z);
            }
        }
    }

    fn gradient_x(chunk: &mut ProtoChunk, x: i32, z: i32) -> i32 {
        chunk.height_at(HeightmapType::WorldSurfaceWg, (x + 1).min(15), z) - chunk.height_at(HeightmapType::WorldSurfaceWg, (x - 1).max(0), z)
    }

    fn gradient_z(chunk: &mut ProtoChunk, x: i32, z: i32) -> i32 {
        chunk.height_at(HeightmapType::WorldSurfaceWg, x, (z + 1).min(15)) - chunk.height_at(HeightmapType::WorldSurfaceWg, x, (z - 1).max(0))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_surface(
        &self,
        state: &RandomState,
        chunk: &mut ProtoChunk,
        samplers: &mut SamplerContext,
        full_volume: DensityVolume,
        biome_getter: &mut dyn FnMut(i32, i32, i32) -> Biome,
        generation: WorldGenerationContext,
        rule: &MaterialRule,
        possible_biomes: Option<&[bool]>,
    ) {
        let (min_x, min_z) = (chunk.pos.min_block_x(), chunk.pos.min_block_z());
        let (min_y, max_y) = (chunk.min_y, chunk.max_y());
        let max_block_y = match chunk.highest_filled_section_index() {
            Some(i) => ((chunk.min_y >> 4) + i as i32) * 16 + 15,
            None => ((chunk.min_y >> 4) - 1) * 16 + 15,
        };
        let narrowed = DensityVolume::new(
            full_volume.size_x,
            (max_block_y - full_volume.min_y + 1).max(1),
            full_volume.size_z,
            full_volume.min_x,
            full_volume.min_y,
            full_volume.min_z,
        );

        let mut context = RuleContext::new(self, state, narrowed, samplers, biome_getter, generation, possible_biomes);
        let mut rule = Rule::compile(rule, &mut context);
        for x in 0..16 {
            for z in 0..16 {
                let block_x = min_x + x;
                let block_z = min_z + z;
                let starting_height = chunk.height_at(HeightmapType::WorldSurfaceWg, x, z) + 1;
                let surface_biome = (context.biome_getter)(block_x, starting_height, block_z);
                if surface_biome == self.eroded_badlands {
                    self.eroded_badlands_extension(chunk, block_x, block_z, starting_height);
                }
                let height = chunk.height_at(HeightmapType::WorldSurfaceWg, x, z) + 1;
                let gradient_x = Self::gradient_x(chunk, x, z);
                let gradient_z = Self::gradient_z(chunk, x, z);
                context.update_xz(block_x, block_z, gradient_x, gradient_z);
                let mut stone_above_depth = 0;
                let mut water_height = i32::MIN;
                let mut next_ceiling_stone_y = i32::MAX;
                let end_y = min_y;
                let mut y = height;
                while y >= end_y {
                    let old = chunk.block_state(x, y, z);
                    if old.is_air() {
                        stone_above_depth = 0;
                        water_height = i32::MIN;
                    } else if !old.fluid_state().is_empty() {
                        if water_height == i32::MIN {
                            water_height = y + 1;
                        }
                    } else {
                        if next_ceiling_stone_y >= y {
                            next_ceiling_stone_y = WAY_BELOW_MIN_Y;
                            let mut lookahead = y - 1;
                            while lookahead >= end_y - 1 {
                                if !Self::is_stone(chunk.block_state(x, lookahead, z)) {
                                    next_ceiling_stone_y = lookahead + 1;
                                    break;
                                }
                                lookahead -= 1;
                            }
                        }
                        stone_above_depth += 1;
                        let stone_below_depth = y - next_ceiling_stone_y + 1;
                        context.update_y(stone_above_depth, stone_below_depth, water_height, y);
                        if y >= min_y && y <= max_y
                            && let Some(new) = rule.try_apply(&mut context, block_x, y, block_z)
                        {
                            Self::set_column(chunk, x, y, z, new);
                        }
                    }
                    y -= 1;
                }
                if surface_biome == self.frozen_ocean || surface_biome == self.deep_frozen_ocean {
                    let min_surface = context.min_surface_level();
                    self.frozen_ocean_extension(chunk, min_surface, surface_biome, block_x, block_z, starting_height);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn top_material(
        &self,
        rule: &MaterialRule,
        state: &RandomState,
        generation: WorldGenerationContext,
        biome_getter: &mut dyn FnMut(i32, i32, i32) -> Biome,
        chunk: &mut ProtoChunk,
        samplers: &mut SamplerContext,
        (x, y, z): (i32, i32, i32),
        under_fluid: bool,
    ) -> Option<BlockState> {
        let volume = DensityVolume::new(1, 1, 1, x, y, z);
        let mut context = RuleContext::new(self, state, volume, samplers, biome_getter, generation, None);
        let mut rule = Rule::compile(rule, &mut context);
        let gradient_x = Self::gradient_x(chunk, x & 15, z & 15);
        let gradient_z = Self::gradient_z(chunk, x & 15, z & 15);
        context.update_xz(x, z, gradient_x, gradient_z);
        context.update_y(1, 1, if under_fluid { y + 1 } else { i32::MIN }, y);
        rule.try_apply(&mut context, x, y, z)
    }

    fn eroded_badlands_extension(&self, chunk: &mut ProtoChunk, block_x: i32, block_z: i32, height: i32) {
        let (x, z) = (block_x & 15, block_z & 15);
        let pillar_buffer = jmin(
            (self.badlands_surface.get(block_x as f64, 0.0, block_z as f64) as f64 * 8.25).abs(),
            (self.badlands_pillar.get(block_x as f64 * 0.2, 0.0, block_z as f64 * 0.2) * 15.0) as f64,
        );
        if pillar_buffer <= 0.0 {
            return;
        }
        let pillar_floor = (self.badlands_pillar_roof.get(block_x as f64 * 0.75, 0.0, block_z as f64 * 0.75) as f64 * 1.5).abs();
        let extension_top = 64.0 + jmin(pillar_buffer * pillar_buffer * 2.5, (pillar_floor * 50.0).ceil() + 24.0);
        let start_y = mth::floor(extension_top);
        if height > start_y {
            return;
        }
        let mut y = start_y;
        while y >= chunk.min_y {
            let old = chunk.block_state(x, y, z);
            if old.is_block(self.default_block.block()) {
                break;
            }
            if old.is_block(blocks::WATER) {
                return;
            }
            y -= 1;
        }
        let mut y = start_y;
        while y >= chunk.min_y && chunk.block_state(x, y, z).is_air() {
            Self::set_column(chunk, x, y, z, self.default_block);
            y -= 1;
        }
    }

    fn frozen_ocean_extension(&self, chunk: &mut ProtoChunk, min_surface_level: i32, biome: Biome, block_x: i32, block_z: i32, height: i32) {
        let (x, z) = (block_x & 15, block_z & 15);
        let iceberg = jmin(
            (self.iceberg_surface.get(block_x as f64, 0.0, block_z as f64) as f64 * 8.25).abs(),
            (self.iceberg_pillar.get(block_x as f64 * 1.28, 0.0, block_z as f64 * 1.28) * 15.0) as f64,
        );
        if iceberg <= 1.8 {
            return;
        }
        let roof = (self.iceberg_pillar_roof.get(block_x as f64 * 1.17, 0.0, block_z as f64 * 1.17) as f64 * 1.5).abs();
        let mut top = jmin(iceberg * iceberg * 1.2, (roof * 40.0).ceil() + 14.0);
        if biome.should_melt_frozen_ocean_iceberg_slightly(BlockPos::new(block_x, self.sea_level, block_z), self.sea_level) {
            top -= 2.0;
        }
        if top <= 2.0 {
            return;
        }
        let extension_bottom = self.sea_level as f64 - top - 7.0;
        top += self.sea_level as f64;
        let extension_top = top;
        let mut random = self.noise_random.at(block_x, 0, block_z);
        let max_snow_depth = 2 + random.next_int_bounded(4);
        let min_snow_height = self.sea_level + 18 + random.next_int_bounded(10);
        let mut snow_depth = 0;
        let mut y = height.max(extension_top as i32 + 1);
        while y >= min_surface_level {
            let place = (chunk.block_state(x, y, z).is_air() && y < extension_top as i32 && random.next_double() > 0.01)
                || (chunk.block_state(x, y, z).is_block(blocks::WATER)
                    && y > extension_bottom as i32
                    && y < self.sea_level
                    && random.next_double() > 0.15);
            if place {
                if snow_depth <= max_snow_depth && y > min_snow_height {
                    Self::set_column(chunk, x, y, z, blocks::SNOW_BLOCK.default_state());
                    snow_depth += 1;
                } else {
                    Self::set_column(chunk, x, y, z, blocks::PACKED_ICE.default_state());
                }
            }
            y -= 1;
        }
    }
}
