use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct RandomBooleanSelectorFeature {
    feature_true: PlacedFeatureHolder,
    feature_false: PlacedFeatureHolder,
}

impl RandomBooleanSelectorFeature {
    pub fn parse(json: &Value) -> Result<RandomBooleanSelectorFeature, String> {
        Ok(RandomBooleanSelectorFeature {
            feature_true: PlacedFeatureHolder::parse(json.get("feature_true").ok_or("missing feature_true")?)?,
            feature_false: PlacedFeatureHolder::parse(json.get("feature_false").ok_or("missing feature_false")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let result = random.next_boolean();
        (if result { &self.feature_true } else { &self.feature_false }).value().place(level, random, origin)
    }
}
