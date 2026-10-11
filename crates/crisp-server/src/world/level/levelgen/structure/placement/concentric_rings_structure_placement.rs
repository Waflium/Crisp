use crisp_core::ChunkPos;
use serde_json::Value;

use super::abstract_spreading_structure_placement::AbstractSpreadingStructurePlacement;
use crate::core::holder_set::HolderSet;
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::levelgen::structure::structure::parse_biome_holder_set;
use crate::world::level::levelgen::structure::structure_set::StructureSetHolder;

pub struct ConcentricRingsStructurePlacement {
    pub spreading: AbstractSpreadingStructurePlacement,
    pub distance: i32,
    pub spread: i32,
    pub count: i32,
    pub preferred_biomes: HolderSet,
}

impl ConcentricRingsStructurePlacement {
    pub fn parse(json: &Value) -> Result<ConcentricRingsStructurePlacement, String> {
        let int = |key: &str| json.get(key).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| format!("missing {key}"));
        Ok(ConcentricRingsStructurePlacement {
            spreading: AbstractSpreadingStructurePlacement::parse(json)?,
            distance: int("distance")?,
            spread: int("spread")?,
            count: int("count")?,
            preferred_biomes: parse_biome_holder_set(json.get("preferred_biomes").ok_or("missing preferred_biomes")?)?,
        })
    }

    pub fn is_placement_chunk(&self, set: StructureSetHolder, state: &ChunkGeneratorStructureState, source_x: i32, source_z: i32) -> bool {
        state.get_ring_positions_for(set).is_some_and(|positions| positions.contains(&ChunkPos::new(source_x, source_z)))
    }
}
