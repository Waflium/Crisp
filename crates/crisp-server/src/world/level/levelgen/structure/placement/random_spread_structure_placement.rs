use crisp_core::ChunkPos;
use serde_json::Value;

use super::abstract_spreading_structure_placement::AbstractSpreadingStructurePlacement;
use super::random_spread_type::RandomSpreadType;
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;

pub struct RandomSpreadStructurePlacement {
    pub spreading: AbstractSpreadingStructurePlacement,
    pub spacing: i32,
    pub separation: i32,
    pub spread_type: RandomSpreadType,
}

impl RandomSpreadStructurePlacement {
    pub fn parse(json: &Value) -> Result<RandomSpreadStructurePlacement, String> {
        let placement = RandomSpreadStructurePlacement {
            spreading: AbstractSpreadingStructurePlacement::parse(json)?,
            spacing: json.get("spacing").and_then(Value::as_i64).ok_or("missing spacing")? as i32,
            separation: json.get("separation").and_then(Value::as_i64).ok_or("missing separation")? as i32,
            spread_type: match json.get("spread_type").and_then(Value::as_str) {
                Some(name) => RandomSpreadType::by_name(name).ok_or_else(|| format!("unknown spread type {name}"))?,
                None => RandomSpreadType::Linear,
            },
        };
        if placement.spacing <= placement.separation {
            return Err("Spacing has to be larger than separation".into());
        }
        Ok(placement)
    }

    pub fn get_potential_structure_chunk(&self, seed: i64, source_x: i32, source_z: i32) -> ChunkPos {
        let spaced_grid_x = source_x.div_euclid(self.spacing);
        let spaced_grid_z = source_z.div_euclid(self.spacing);
        let mut random = WorldgenRandom::legacy(0);
        random.set_large_feature_with_salt(seed, spaced_grid_x, spaced_grid_z, self.spreading.salt);
        let limit = self.spacing - self.separation;
        let spread_x = self.spread_type.evaluate(&mut random, limit);
        let spread_z = self.spread_type.evaluate(&mut random, limit);
        ChunkPos::new(spaced_grid_x * self.spacing + spread_x, spaced_grid_z * self.spacing + spread_z)
    }

    pub fn is_placement_chunk(&self, state: &ChunkGeneratorStructureState, source_x: i32, source_z: i32) -> bool {
        let chunk_pos = self.get_potential_structure_chunk(state.get_level_seed(), source_x, source_z);
        chunk_pos.x == source_x && chunk_pos.z == source_z
    }
}
