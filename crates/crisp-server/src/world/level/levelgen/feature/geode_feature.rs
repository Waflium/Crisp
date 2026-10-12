use std::sync::LazyLock;

use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{FACING, WATERLOGGED};
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::AlgorithmRandom;
use crate::world::level::levelgen::geode_block_settings::GeodeBlockSettings;
use crate::world::level::levelgen::geode_crack_settings::GeodeCrackSettings;
use crate::world::level::levelgen::geode_layer_settings::GeodeLayerSettings;
use crate::world::level::levelgen::synth::NormalNoise;

static NOISE_PARAMETERS: LazyLock<NormalNoise> = LazyLock::new(|| NormalNoise::create_parity(-4, &[1.0]));

pub struct GeodeFeature {
    block_settings: GeodeBlockSettings,
    layer_settings: GeodeLayerSettings,
    crack_settings: GeodeCrackSettings,
    use_potential_placements_chance: f64,
    use_alternate_layer0_chance: f64,
    placements_require_layer0_alternate: bool,
    outer_wall_distance: IntProvider,
    distribution_points: IntProvider,
    point_offset: IntProvider,
    min_gen_offset: i32,
    max_gen_offset: i32,
    noise_multiplier: f64,
    invalid_blocks_threshold: i32,
}

impl GeodeFeature {
    pub fn parse(json: &Value) -> Result<GeodeFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let double = |key: &str, default: f64| json.get(key).and_then(Value::as_f64).unwrap_or(default);
        let int = |key: &str, default: i32| json.get(key).and_then(Value::as_i64).map_or(default, |v| v as i32);
        let provider = |key: &str, min: i32, max: i32| -> Result<IntProvider, String> {
            match json.get(key) {
                Some(value) => IntProvider::parse(value),
                None => Ok(IntProvider::Uniform { min_inclusive: min, max_inclusive: max }),
            }
        };
        Ok(GeodeFeature {
            block_settings: GeodeBlockSettings::parse(field("blocks")?)?,
            layer_settings: GeodeLayerSettings::parse(field("layers")?),
            crack_settings: GeodeCrackSettings::parse(field("crack")?),
            use_potential_placements_chance: double("use_potential_placements_chance", 0.35),
            use_alternate_layer0_chance: double("use_alternate_layer0_chance", 0.0),
            placements_require_layer0_alternate: json.get("placements_require_layer0_alternate").and_then(Value::as_bool).unwrap_or(true),
            outer_wall_distance: provider("outer_wall_distance", 4, 5)?,
            distribution_points: provider("distribution_points", 3, 4)?,
            point_offset: provider("point_offset", 1, 2)?,
            min_gen_offset: int("min_gen_offset", -16),
            max_gen_offset: int("max_gen_offset", 16),
            noise_multiplier: double("noise_multiplier", 0.05),
            invalid_blocks_threshold: field("invalid_blocks_threshold")?.as_i64().ok_or("bad invalid_blocks_threshold")? as i32,
        })
    }

    fn safe_set_block(&self, level: &mut impl WorldGenLevel, pos: BlockPos, state: BlockState) {
        if !self.block_settings.cannot_replace.contains(level.get_block_state(pos).block()) {
            level.set_block(pos, state, 2);
        }
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut points: Vec<(BlockPos, i32)> = Vec::new();
        let num_points = self.distribution_points.sample(random);
        let noise = NOISE_PARAMETERS.create(&mut AlgorithmRandom::Legacy(LegacyRandomSource::new(level.get_seed())));
        let mut crack_points: Vec<BlockPos> = Vec::new();
        let crack_size_adjustment = num_points as f64 / self.outer_wall_distance.max_inclusive() as f64;
        let layers = &self.layer_settings;
        let inner_air = 1.0 / layers.filling.sqrt();
        let innermost_block_layer = 1.0 / (layers.inner_layer + crack_size_adjustment).sqrt();
        let inner_crust = 1.0 / (layers.middle_layer + crack_size_adjustment).sqrt();
        let outer_crust = 1.0 / (layers.outer_layer + crack_size_adjustment).sqrt();
        let crack_size = 1.0 / (self.crack_settings.base_crack_size + random.next_double() / 2.0 + if num_points > 3 { crack_size_adjustment } else { 0.0 }).sqrt();
        let should_generate_crack = (random.next_float() as f64) < self.crack_settings.generate_crack_chance;
        let mut num_invalid_points = 0;
        for _ in 0..num_points {
            let x = self.outer_wall_distance.sample(random);
            let y = self.outer_wall_distance.sample(random);
            let z = self.outer_wall_distance.sample(random);
            let pos = origin.offset(x, y, z);
            let state = level.get_block_state(pos);
            if state.is_air() || self.block_settings.invalid_blocks.contains(state.block()) {
                num_invalid_points += 1;
                if num_invalid_points > self.invalid_blocks_threshold {
                    return false;
                }
            }
            points.push((pos, self.point_offset.sample(random)));
        }
        if should_generate_crack {
            let offset_index = random.next_int_bounded(4);
            let crack_offset = num_points * 2 + 1;
            let (dx, dz) = match offset_index {
                0 => (crack_offset, 0),
                1 => (0, crack_offset),
                2 => (crack_offset, crack_offset),
                _ => (0, 0),
            };
            crack_points.push(origin.offset(dx, 7, dz));
            crack_points.push(origin.offset(dx, 5, dz));
            crack_points.push(origin.offset(dx, 1, dz));
        }
        let mut potential_crystal_placements: Vec<BlockPos> = Vec::new();
        let (min, max) = (self.min_gen_offset, self.max_gen_offset);
        for z in origin.z + min..=origin.z + max {
            for y in origin.y + min..=origin.y + max {
                for x in origin.x + min..=origin.x + max {
                    let point_inside = BlockPos::new(x, y, z);
                    let noise_offset = noise.get(x as f64, y as f64, z as f64) as f64 * self.noise_multiplier;
                    let mut dist_sum_shell = 0.0;
                    for &(point, offset) in &points {
                        dist_sum_shell += inv_sqrt(point_inside.dist_sqr(point) + offset as f64) + noise_offset;
                    }
                    if dist_sum_shell < outer_crust {
                        continue;
                    }
                    if dist_sum_shell >= inner_air {
                        let state = self.block_settings.filling_provider.value().get_state(level, random, point_inside);
                        self.safe_set_block(level, point_inside, state);
                        continue;
                    }
                    let mut dist_sum_crack = 0.0;
                    for &point in &crack_points {
                        dist_sum_crack += inv_sqrt(point_inside.dist_sqr(point) + self.crack_settings.crack_point_offset as f64) + noise_offset;
                    }
                    if should_generate_crack && dist_sum_crack >= crack_size {
                        self.safe_set_block(level, point_inside, blocks::AIR.default_state());
                        for direction in Direction::VALUES {
                            let adjacent_pos = point_inside.relative(direction);
                            let adjacent_fluid_state = level.get_fluid_state(adjacent_pos);
                            if !adjacent_fluid_state.is_empty() {
                                level.schedule_fluid_tick(adjacent_pos, adjacent_fluid_state.fluid, 0);
                            }
                        }
                    } else if dist_sum_shell >= innermost_block_layer {
                        let use_alternate_layer = (random.next_float() as f64) < self.use_alternate_layer0_chance;
                        let provider = if use_alternate_layer { &self.block_settings.alternate_inner_layer_provider } else { &self.block_settings.inner_layer_provider };
                        let state = provider.value().get_state(level, random, point_inside);
                        self.safe_set_block(level, point_inside, state);
                        if (!self.placements_require_layer0_alternate || use_alternate_layer) && (random.next_float() as f64) < self.use_potential_placements_chance {
                            potential_crystal_placements.push(point_inside);
                        }
                    } else if dist_sum_shell >= inner_crust {
                        let state = self.block_settings.middle_layer_provider.value().get_state(level, random, point_inside);
                        self.safe_set_block(level, point_inside, state);
                    } else if dist_sum_shell >= outer_crust {
                        let state = self.block_settings.outer_layer_provider.value().get_state(level, random, point_inside);
                        self.safe_set_block(level, point_inside, state);
                    }
                }
            }
        }
        let inner_placements = &self.block_settings.inner_placements;
        for crystal_pos in potential_crystal_placements {
            let mut block_state = inner_placements[random.next_int_bounded(inner_placements.len() as i32) as usize];
            for direction in Direction::VALUES {
                if block_state.has(FACING) {
                    block_state = block_state.set(FACING, direction);
                }
                let place_pos = crystal_pos.relative(direction);
                let place_state = level.get_block_state(place_pos);
                if block_state.has(WATERLOGGED) {
                    block_state = block_state.set(WATERLOGGED, place_state.fluid_state().is_source());
                }
                if can_cluster_grow_at_state(place_state) {
                    self.safe_set_block(level, place_pos, block_state);
                    break;
                }
            }
        }
        true
    }
}

fn inv_sqrt(x: f64) -> f64 {
    1.0 / x.sqrt()
}

fn can_cluster_grow_at_state(state: BlockState) -> bool {
    state.is_air() || state.is_block(blocks::WATER) && state.fluid_state().amount == 8
}
