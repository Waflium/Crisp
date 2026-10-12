use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct RandomBooleanSelectorFeature {
    feature_true: PlacedFeatureHolder,
    feature_false: PlacedFeatureHolder,
}

impl RandomBooleanSelectorFeature {
    pub fn get_sub_features(&self, out: &mut Vec<u32>) {
        self.feature_true.value().get_features(out);
        self.feature_false.value().get_features(out);
    }

    pub fn parse(json: &Value) -> Result<RandomBooleanSelectorFeature, String> {
        Ok(RandomBooleanSelectorFeature {
            feature_true: PlacedFeatureHolder::parse(json.get("feature_true").ok_or("missing feature_true")?)?,
            feature_false: PlacedFeatureHolder::parse(json.get("feature_false").ok_or("missing feature_false")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let result = random.next_boolean();
        (if result { &self.feature_true } else { &self.feature_false }).value().place(level, random, origin)
    }
}
