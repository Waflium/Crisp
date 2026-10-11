use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;
use serde_json::Value;

use super::placement_context::PlacementContext;
use crate::util::valueproviders::{IntProvider, float};
use crate::world::level::biome::BIOME_INFO_NOISE;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;
use crate::world::level::levelgen::heightproviders::HeightProvider;

pub enum PlacementModifier {
    Biome,
    BlockPredicateFilter(BlockPredicate),
    Count(IntProvider),
    CountOnEveryLayer(IntProvider),
    EnvironmentScan { direction_of_search: Direction, target_condition: BlockPredicate, allowed_search_condition: BlockPredicate, max_steps: i32 },
    FixedPlacement(Vec<BlockPos>),
    HeightRange(HeightProvider),
    Heightmap(HeightmapType),
    InSquare,
    NoiseBasedCount { noise_to_count_ratio: i32, noise_factor: f64, noise_offset: f64 },
    NoiseThresholdCount { noise_level: f64, below_noise: i32, above_noise: i32 },
    Offset { x: IntProvider, y: IntProvider, z: IntProvider },
    Cuboid { xz_size: IntProvider, y_size: IntProvider, include_edges: bool, include_interior: bool },
    RandomChance(f32),
    RandomlySelected(Vec<PlacementModifier>),
    RarityFilter(i32),
    SurfaceRelativeThresholdFilter { heightmap: HeightmapType, min_inclusive: i32, max_inclusive: i32 },
    SurfaceWaterDepthFilter(i32),
}

pub fn parse_heightmap(name: &str) -> Result<HeightmapType, String> {
    Ok(match name {
        "WORLD_SURFACE_WG" => HeightmapType::WorldSurfaceWg,
        "WORLD_SURFACE" => HeightmapType::WorldSurface,
        "OCEAN_FLOOR_WG" => HeightmapType::OceanFloorWg,
        "OCEAN_FLOOR" => HeightmapType::OceanFloor,
        "MOTION_BLOCKING" => HeightmapType::MotionBlocking,
        "MOTION_BLOCKING_NO_LEAVES" => HeightmapType::MotionBlockingNoLeaves,
        other => return Err(format!("unknown heightmap {other}")),
    })
}

impl PlacementModifier {
    pub fn parse(json: &Value) -> Result<PlacementModifier, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("placement modifier without type")?;
        let get = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let int = |key: &str| get(key).and_then(|v| v.as_i64().map(|v| v as i32).ok_or_else(|| format!("{key} is not an int")));
        let double = |key: &str| get(key).and_then(|v| v.as_f64().ok_or_else(|| format!("{key} is not a number")));
        let int_provider = |key: &str| get(key).and_then(IntProvider::parse);
        let predicate = |key: &str| get(key).and_then(BlockPredicate::parse);
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "biome" => PlacementModifier::Biome,
            "block_predicate_filter" => PlacementModifier::BlockPredicateFilter(predicate("predicate")?),
            "count" => PlacementModifier::Count(int_provider("count")?),
            "count_on_every_layer" => PlacementModifier::CountOnEveryLayer(int_provider("count")?),
            "environment_scan" => PlacementModifier::EnvironmentScan {
                direction_of_search: get("direction_of_search")?.as_str().and_then(Direction::by_name).ok_or("bad direction_of_search")?,
                target_condition: predicate("target_condition")?,
                allowed_search_condition: json.get("allowed_search_condition").map_or(Ok(BlockPredicate::True), BlockPredicate::parse)?,
                max_steps: int("max_steps")?,
            },
            "fixed_placement" => PlacementModifier::FixedPlacement(
                get("positions")?
                    .as_array()
                    .ok_or("positions")?
                    .iter()
                    .map(|p| {
                        let v = p.as_array().filter(|v| v.len() == 3).ok_or("a position")?;
                        Ok(BlockPos::new(v[0].as_i64().unwrap_or(0) as i32, v[1].as_i64().unwrap_or(0) as i32, v[2].as_i64().unwrap_or(0) as i32))
                    })
                    .collect::<Result<_, String>>()?,
            ),
            "height_range" => PlacementModifier::HeightRange(HeightProvider::parse(get("height")?)?),
            "heightmap" => PlacementModifier::Heightmap(parse_heightmap(get("heightmap")?.as_str().ok_or("heightmap")?)?),
            "in_square" => PlacementModifier::InSquare,
            "noise_based_count" => PlacementModifier::NoiseBasedCount {
                noise_to_count_ratio: int("noise_to_count_ratio")?,
                noise_factor: double("noise_factor")?,
                noise_offset: json.get("noise_offset").and_then(Value::as_f64).unwrap_or(0.0),
            },
            "noise_threshold_count" => PlacementModifier::NoiseThresholdCount {
                noise_level: double("noise_level")?,
                below_noise: int("below_noise")?,
                above_noise: int("above_noise")?,
            },
            "offset" => PlacementModifier::Offset { x: int_provider("x")?, y: int_provider("y")?, z: int_provider("z")? },
            "cuboid" => PlacementModifier::Cuboid {
                xz_size: int_provider("xz_size")?,
                y_size: int_provider("y_size")?,
                include_edges: json.get("include_edges").and_then(Value::as_bool).unwrap_or(true),
                include_interior: json.get("include_interior").and_then(Value::as_bool).unwrap_or(true),
            },
            "random_chance" => PlacementModifier::RandomChance(float(get("chance")?)?),
            "randomly_selected" => PlacementModifier::RandomlySelected(get("placements")?.as_array().ok_or("placements")?.iter().map(PlacementModifier::parse).collect::<Result<_, _>>()?),
            "rarity_filter" => PlacementModifier::RarityFilter(int("chance")?),
            "surface_relative_threshold_filter" => PlacementModifier::SurfaceRelativeThresholdFilter {
                heightmap: parse_heightmap(get("heightmap")?.as_str().ok_or("heightmap")?)?,
                min_inclusive: json.get("min_inclusive").and_then(Value::as_i64).map_or(i32::MIN, |v| v as i32),
                max_inclusive: json.get("max_inclusive").and_then(Value::as_i64).map_or(i32::MAX, |v| v as i32),
            },
            "surface_water_depth_filter" => PlacementModifier::SurfaceWaterDepthFilter(int("max_water_depth")?),
            other => return Err(format!("unknown placement modifier {other}")),
        })
    }

    pub fn modify<R: RandomSource>(&self, context: &mut PlacementContext, random: &mut R, origin: BlockPos, output: &mut Vec<BlockPos>) {
        match self {
            PlacementModifier::Biome => {
                if context.biome_has_top_feature(origin) {
                    output.push(origin);
                }
            }
            PlacementModifier::BlockPredicateFilter(predicate) => {
                if predicate.test(context.level, origin) {
                    output.push(origin);
                }
            }
            PlacementModifier::Count(count) => {
                for _ in 0..count.sample(random) {
                    output.push(origin);
                }
            }
            PlacementModifier::CountOnEveryLayer(count) => {
                let mut layer = 0;
                loop {
                    let mut found_any = false;
                    let mut i = 0;
                    while i < count.sample(random) {
                        let x = random.next_int_bounded(16) + origin.x;
                        let z = random.next_int_bounded(16) + origin.z;
                        let start_y = context.get_height(HeightmapType::MotionBlocking, x, z);
                        let y = find_on_ground_y_position(context, x, start_y, z, layer);
                        if y != i32::MAX {
                            output.push(BlockPos::new(x, y, z));
                            found_any = true;
                        }
                        i += 1;
                    }
                    layer += 1;
                    if !found_any {
                        break;
                    }
                }
            }
            PlacementModifier::EnvironmentScan { direction_of_search, target_condition, allowed_search_condition, max_steps } => {
                let mut pos = origin;
                let level = &mut *context.level;
                if !allowed_search_condition.test(level, pos) {
                    return;
                }
                for _ in 0..*max_steps {
                    if target_condition.test(level, pos) {
                        output.push(pos);
                        return;
                    }
                    pos = pos.relative(*direction_of_search);
                    if level.is_outside_build_height(pos.y) {
                        return;
                    }
                    if !allowed_search_condition.test(level, pos) {
                        break;
                    }
                }
                if target_condition.test(level, pos) {
                    output.push(pos);
                }
            }
            PlacementModifier::FixedPlacement(positions) => {
                let (chunk_x, chunk_z) = (origin.x >> 4, origin.z >> 4);
                for &position in positions {
                    if position.x >> 4 == chunk_x && position.z >> 4 == chunk_z {
                        output.push(position);
                    }
                }
            }
            PlacementModifier::HeightRange(height) => output.push(BlockPos::new(origin.x, height.sample(random, &context.generation), origin.z)),
            PlacementModifier::Heightmap(heightmap) => {
                let height = context.get_height(*heightmap, origin.x, origin.z);
                if height > context.get_min_y() {
                    output.push(BlockPos::new(origin.x, height, origin.z));
                }
            }
            PlacementModifier::InSquare => {
                let x = random.next_int_bounded(16) + origin.x;
                let z = random.next_int_bounded(16) + origin.z;
                output.push(BlockPos::new(x, origin.y, z));
            }
            PlacementModifier::NoiseBasedCount { noise_to_count_ratio, noise_factor, noise_offset } => {
                let flower_noise = BIOME_INFO_NOISE.get_2d(origin.x as f64 / noise_factor, origin.z as f64 / noise_factor) as f64;
                let count = ((flower_noise + noise_offset) * *noise_to_count_ratio as f64).ceil() as i32;
                for _ in 0..count {
                    output.push(origin);
                }
            }
            PlacementModifier::NoiseThresholdCount { noise_level, below_noise, above_noise } => {
                let flower_noise = BIOME_INFO_NOISE.get_2d(origin.x as f64 / 200.0, origin.z as f64 / 200.0) as f64;
                let count = if flower_noise < *noise_level { *below_noise } else { *above_noise };
                for _ in 0..count {
                    output.push(origin);
                }
            }
            PlacementModifier::Offset { x, y, z } => {
                let dx = x.sample(random);
                let dy = y.sample(random);
                let dz = z.sample(random);
                output.push(origin.offset(dx, dy, dz));
            }
            PlacementModifier::Cuboid { xz_size, y_size, include_edges, include_interior } => {
                let height = y_size.sample(random);
                let width = xz_size.sample(random);
                let length = xz_size.sample(random);
                for x in 0..=width {
                    for y in 0..=height {
                        for z in 0..=length {
                            if (*include_edges || x != 0 && x != width || y != 0 && y != height)
                                && (*include_edges || z != 0 && z != length || y != 0 && y != height)
                                && (*include_edges || x != 0 && x != width || z != 0 && z != length)
                                && (*include_interior || x == 0 || x == width || y == 0 || y == height || z == 0 || z == length)
                            {
                                output.push(origin.offset(x, y, z));
                            }
                        }
                    }
                }
            }
            PlacementModifier::RandomChance(chance) => {
                if random.next_float() < *chance {
                    output.push(origin);
                }
            }
            PlacementModifier::RandomlySelected(placements) => {
                let index = random.next_int_bounded(placements.len() as i32) as usize;
                placements[index].modify(context, random, origin, output);
            }
            PlacementModifier::RarityFilter(chance) => {
                if random.next_float() < 1.0 / *chance as f32 {
                    output.push(origin);
                }
            }
            PlacementModifier::SurfaceRelativeThresholdFilter { heightmap, min_inclusive, max_inclusive } => {
                let surface_y = context.get_height(*heightmap, origin.x, origin.z) as i64;
                let min_y = surface_y + *min_inclusive as i64;
                let max_y = surface_y + *max_inclusive as i64;
                if min_y <= origin.y as i64 && origin.y as i64 <= max_y {
                    output.push(origin);
                }
            }
            PlacementModifier::SurfaceWaterDepthFilter(max_water_depth) => {
                let y_ocean_floor = context.get_height(HeightmapType::OceanFloor, origin.x, origin.z);
                let y_surface_floor = context.get_height(HeightmapType::WorldSurface, origin.x, origin.z);
                if y_surface_floor - y_ocean_floor <= *max_water_depth {
                    output.push(origin);
                }
            }
        }
    }
}

fn find_on_ground_y_position(context: &mut PlacementContext, x_start: i32, y_start: i32, z_start: i32, layer_to_place_on: i32) -> i32 {
    let mut current_layer = 0;
    let mut current_block = context.get_block_state(BlockPos::new(x_start, y_start, z_start));
    let mut y = y_start;
    while y >= context.get_min_y() + 1 {
        let below_block = context.get_block_state(BlockPos::new(x_start, y - 1, z_start));
        if !is_empty(below_block) && is_empty(current_block) && !below_block.is_block(blocks::BEDROCK) {
            if current_layer == layer_to_place_on {
                return y;
            }
            current_layer += 1;
        }
        current_block = below_block;
        y -= 1;
    }
    i32::MAX
}

fn is_empty(block_state: crisp_data::BlockState) -> bool {
    block_state.is_air() || block_state.is_block(blocks::WATER) || block_state.is_block(blocks::LAVA)
}
