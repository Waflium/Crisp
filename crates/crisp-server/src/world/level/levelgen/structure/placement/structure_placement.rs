use serde_json::Value;

use super::concentric_rings_structure_placement::ConcentricRingsStructurePlacement;
use super::random_spread_structure_placement::RandomSpreadStructurePlacement;
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::levelgen::structure::structure_set::StructureSetHolder;

pub enum StructurePlacement {
    RandomSpread(RandomSpreadStructurePlacement),
    ConcentricRings(ConcentricRingsStructurePlacement),
    DimensionOrigin,
}

impl StructurePlacement {
    pub fn parse(json: &Value) -> Result<StructurePlacement, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("placement without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "random_spread" => StructurePlacement::RandomSpread(RandomSpreadStructurePlacement::parse(json)?),
            "concentric_rings" => StructurePlacement::ConcentricRings(ConcentricRingsStructurePlacement::parse(json)?),
            "dimension_origin" => StructurePlacement::DimensionOrigin,
            other => return Err(format!("unknown structure placement {other}")),
        })
    }

    pub fn is_structure_chunk(&self, set: StructureSetHolder, state: &ChunkGeneratorStructureState, source_x: i32, source_z: i32) -> bool {
        let spreading = match self {
            StructurePlacement::RandomSpread(placement) => {
                if !placement.is_placement_chunk(state, source_x, source_z) {
                    return false;
                }
                &placement.spreading
            }
            StructurePlacement::ConcentricRings(placement) => {
                if !placement.is_placement_chunk(set, state, source_x, source_z) {
                    return false;
                }
                &placement.spreading
            }
            StructurePlacement::DimensionOrigin => {
                let origin = state.get_dimension_origin();
                return origin.x == source_x && origin.z == source_z;
            }
        };
        spreading.apply_additional_chunk_restrictions(source_x, source_z, state.get_level_seed())
            && spreading.apply_interactions_with_other_structures(state, source_x, source_z)
    }
}
