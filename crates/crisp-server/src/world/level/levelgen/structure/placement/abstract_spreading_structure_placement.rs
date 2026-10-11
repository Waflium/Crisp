use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::levelgen::structure::structure_set::StructureSetHolder;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;

const HIGHLY_ARBITRARY_RANDOM_SALT: i32 = 10387320;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrequencyReductionMethod {
    Default,
    LegacyType1,
    LegacyType2,
    LegacyType3,
}

impl FrequencyReductionMethod {
    fn by_name(name: &str) -> Option<FrequencyReductionMethod> {
        Some(match name {
            "default" => FrequencyReductionMethod::Default,
            "legacy_type_1" => FrequencyReductionMethod::LegacyType1,
            "legacy_type_2" => FrequencyReductionMethod::LegacyType2,
            "legacy_type_3" => FrequencyReductionMethod::LegacyType3,
            _ => return None,
        })
    }

    pub fn should_generate(self, seed: i64, salt: i32, source_x: i32, source_z: i32, probability: f32) -> bool {
        match self {
            FrequencyReductionMethod::Default => probability_reducer(seed, salt, source_x, source_z, probability),
            FrequencyReductionMethod::LegacyType1 => legacy_pillager_outpost_reducer(seed, salt, source_x, source_z, probability),
            FrequencyReductionMethod::LegacyType2 => legacy_arbitrary_salt_probability_reducer(seed, salt, source_x, source_z, probability),
            FrequencyReductionMethod::LegacyType3 => legacy_probability_reducer_with_double(seed, salt, source_x, source_z, probability),
        }
    }
}

fn probability_reducer(seed: i64, salt: i32, source_x: i32, source_z: i32, probability: f32) -> bool {
    let mut random = WorldgenRandom::legacy(0);
    random.set_large_feature_with_salt(seed, salt, source_x, source_z);
    random.next_float() < probability
}

fn legacy_probability_reducer_with_double(seed: i64, _salt: i32, source_x: i32, source_z: i32, probability: f32) -> bool {
    let mut random = WorldgenRandom::legacy(0);
    random.set_large_feature_seed(seed, source_x, source_z);
    random.next_double() < probability as f64
}

fn legacy_arbitrary_salt_probability_reducer(seed: i64, _salt: i32, source_x: i32, source_z: i32, probability: f32) -> bool {
    let mut random = WorldgenRandom::legacy(0);
    random.set_large_feature_with_salt(seed, source_x, source_z, HIGHLY_ARBITRARY_RANDOM_SALT);
    random.next_float() < probability
}

fn legacy_pillager_outpost_reducer(seed: i64, _salt: i32, source_x: i32, source_z: i32, probability: f32) -> bool {
    let cx = source_x >> 4;
    let cz = source_z >> 4;
    let mut random = WorldgenRandom::legacy(0);
    random.set_seed((cx ^ (cz << 4)) as i64 ^ seed);
    random.next_int();
    random.next_int_bounded((1.0 / probability) as i32) == 0
}

pub struct ExclusionZone {
    pub other_set: StructureSetHolder,
    pub chunk_count: i32,
}

impl ExclusionZone {
    fn is_placement_forbidden(&self, state: &ChunkGeneratorStructureState, source_x: i32, source_z: i32) -> bool {
        state.has_structure_chunk_in_range(self.other_set, source_x, source_z, self.chunk_count)
    }
}

pub struct AbstractSpreadingStructurePlacement {
    pub locate_offset: BlockPos,
    pub frequency_reduction_method: FrequencyReductionMethod,
    pub frequency: f32,
    pub salt: i32,
    pub exclusion_zone: Option<ExclusionZone>,
}

impl AbstractSpreadingStructurePlacement {
    pub fn parse(json: &Value) -> Result<AbstractSpreadingStructurePlacement, String> {
        let locate_offset = match json.get("locate_offset").and_then(Value::as_array) {
            Some(v) if v.len() == 3 => BlockPos::new(v[0].as_i64().unwrap_or(0) as i32, v[1].as_i64().unwrap_or(0) as i32, v[2].as_i64().unwrap_or(0) as i32),
            _ => BlockPos::ZERO,
        };
        let exclusion_zone = match json.get("exclusion_zone") {
            Some(zone) => {
                let other = zone.get("other_set").and_then(Value::as_str).ok_or("missing other_set")?;
                Some(ExclusionZone {
                    other_set: StructureSetHolder::by_name(other).ok_or_else(|| format!("unknown structure set {other}"))?,
                    chunk_count: zone.get("chunk_count").and_then(Value::as_i64).ok_or("missing chunk_count")? as i32,
                })
            }
            None => None,
        };
        Ok(AbstractSpreadingStructurePlacement {
            locate_offset,
            frequency_reduction_method: match json.get("frequency_reduction_method").and_then(Value::as_str) {
                Some(name) => FrequencyReductionMethod::by_name(name).ok_or_else(|| format!("unknown frequency reduction method {name}"))?,
                None => FrequencyReductionMethod::Default,
            },
            frequency: json.get("frequency").and_then(Value::as_f64).unwrap_or(1.0) as f32,
            salt: json.get("salt").and_then(Value::as_i64).ok_or("missing salt")? as i32,
            exclusion_zone,
        })
    }

    pub fn apply_additional_chunk_restrictions(&self, source_x: i32, source_z: i32, level_seed: i64) -> bool {
        !(self.frequency < 1.0) || self.frequency_reduction_method.should_generate(level_seed, self.salt, source_x, source_z, self.frequency)
    }

    pub fn apply_interactions_with_other_structures(&self, state: &ChunkGeneratorStructureState, source_x: i32, source_z: i32) -> bool {
        self.exclusion_zone.as_ref().is_none_or(|zone| !zone.is_placement_forbidden(state, source_x, source_z))
    }
}
