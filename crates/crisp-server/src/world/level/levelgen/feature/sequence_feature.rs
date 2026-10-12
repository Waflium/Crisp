use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::placement::feature_placer::FeaturePlacer;
use crate::world::level::levelgen::placement::placed_feature::{PlacedFeatureHolder, parse_placed_feature_set};

pub struct SequenceFeature {
    features: Vec<PlacedFeatureHolder>,
}

impl SequenceFeature {
    pub fn parse(json: &Value) -> Result<SequenceFeature, String> {
        Ok(SequenceFeature { features: parse_placed_feature_set(json.get("features").ok_or("missing features")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut placer = FeaturePlacer::default();
        for feature in &self.features {
            if !placer.place(level, feature.value(), random, origin) {
                return false;
            }
        }
        true
    }
}
