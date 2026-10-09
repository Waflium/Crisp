mod canyon_world_carver;
mod cave_world_carver;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use crisp_core::random::LegacyRandomSource;
use crisp_core::{ChunkPos, RandomSource, mth};
use crisp_data::SyncedRegistry;
use serde_json::Value;

pub use canyon_world_carver::CanyonWorldCarver;
pub use cave_world_carver::CaveWorldCarver;

use super::material::WorldGenerationContext;
use crate::world::level::chunk::carving_mask::CarvingMask;

pub enum WorldCarver {
    Cave(CaveWorldCarver),
    Canyon(CanyonWorldCarver),
}

impl WorldCarver {
    pub fn parse(json: &Value) -> Result<WorldCarver, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("carver without type")?;
        Ok(match ty {
            "minecraft:cave" => WorldCarver::Cave(CaveWorldCarver::parse(json)?),
            "minecraft:canyon" => WorldCarver::Canyon(CanyonWorldCarver::parse(json)?),
            other => return Err(format!("unknown carver {other}")),
        })
    }

    pub fn range(&self) -> i32 {
        4
    }

    pub fn is_start_chunk(&self, random: &mut impl RandomSource) -> bool {
        let probability = match self {
            WorldCarver::Cave(c) => c.probability,
            WorldCarver::Canyon(c) => c.probability,
        };
        random.next_float() <= probability
    }

    pub fn carve(&self, context: &WorldGenerationContext, random: &mut impl RandomSource, chunk: ChunkPos, source: ChunkPos, mask: &mut CarvingMask) {
        match self {
            WorldCarver::Cave(c) => c.carve(self.range(), context, random, chunk, source, mask),
            WorldCarver::Canyon(c) => c.carve(self.range(), context, random, chunk, source, mask),
        }
    }
}

pub static BIOME_CARVERS: LazyLock<Vec<Vec<Arc<WorldCarver>>>> = LazyLock::new(|| load_biome_carvers().expect("vanilla carvers"));

fn load_biome_carvers() -> Result<Vec<Vec<Arc<WorldCarver>>>, String> {
    let mut carvers = HashMap::new();
    for &(name, source) in crisp_data::CARVERS {
        let json: Value = serde_json::from_str(source).map_err(|e| format!("{name}: {e}"))?;
        carvers.insert(name, Arc::new(WorldCarver::parse(&json).map_err(|e| format!("{name}: {e}"))?));
    }
    let biomes: HashMap<&str, &str> = crisp_data::BIOMES.iter().copied().collect();
    SyncedRegistry::get("minecraft:worldgen/biome")
        .entries
        .iter()
        .map(|&biome| {
            let json: Value = serde_json::from_str(biomes[biome]).map_err(|e| format!("{biome}: {e}"))?;
            let names: Vec<&str> = match &json["carvers"] {
                Value::String(name) => vec![name],
                Value::Array(names) => names.iter().map(|n| n.as_str().ok_or("bad carver")).collect::<Result<_, _>>()?,
                other => return Err(format!("{biome}: bad carvers {other}")),
            };
            names.into_iter().map(|name| carvers.get(name).cloned().ok_or_else(|| format!("{biome}: unknown carver {name}"))).collect()
        })
        .collect()
}

fn middle_block(chunk: i32) -> i32 {
    (chunk << 4) + 8
}

#[allow(clippy::too_many_arguments)]
fn carve_ellipsoid(
    chunk: ChunkPos,
    x: f64,
    y: f64,
    z: f64,
    horizontal_radius: f64,
    vertical_radius: f64,
    mask: &mut CarvingMask,
    mut should_skip: impl FnMut(f64, f64, f64, i32) -> bool,
) {
    let center_x = middle_block(chunk.x) as f64;
    let center_z = middle_block(chunk.z) as f64;
    let max_delta = 16.0 + horizontal_radius * 2.0;
    if (x - center_x).abs() > max_delta || (z - center_z).abs() > max_delta {
        return;
    }
    let (chunk_min_x, chunk_min_z) = (chunk.min_block_x(), chunk.min_block_z());
    let min_x_index = (mth::floor(x - horizontal_radius) - chunk_min_x - 1).max(0);
    let max_x_index = (mth::floor(x + horizontal_radius) - chunk_min_x).min(15);
    let min_y = (mth::floor(y - vertical_radius) - 1).max(mask.min_y());
    let max_y = (mth::floor(y + vertical_radius) + 1).min(mask.max_y());
    let min_z_index = (mth::floor(z - horizontal_radius) - chunk_min_z - 1).max(0);
    let max_z_index = (mth::floor(z + horizontal_radius) - chunk_min_z).min(15);
    for x_index in min_x_index..=max_x_index {
        let world_x = chunk_min_x + x_index;
        let xd = (world_x as f64 + 0.5 - x) / horizontal_radius;
        for z_index in min_z_index..=max_z_index {
            let world_z = chunk_min_z + z_index;
            let zd = (world_z as f64 + 0.5 - z) / horizontal_radius;
            if xd * xd + zd * zd >= 1.0 {
                continue;
            }
            let mut world_y = max_y;
            while world_y > min_y {
                let yd = (world_y as f64 - 0.5 - y) / vertical_radius;
                if !should_skip(xd, yd, zd, world_y) {
                    mask.carve(x_index, world_y, z_index);
                }
                world_y -= 1;
            }
        }
    }
}

fn can_reach(chunk: ChunkPos, x: f64, z: f64, current_step: i32, total_steps: i32, thickness: f32) -> bool {
    let xd = x - middle_block(chunk.x) as f64;
    let zd = z - middle_block(chunk.z) as f64;
    let remaining = (total_steps - current_step) as f64;
    let rr = (thickness + 2.0 + 16.0) as f64;
    xd * xd + zd * zd - remaining * remaining <= rr * rr
}

fn tunnel_random(seed: i64) -> LegacyRandomSource {
    LegacyRandomSource::new(seed)
}
