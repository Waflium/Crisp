use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::random::WeightedList;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct WeightedRandomSelectorFeature {
    features: WeightedList<PlacedFeatureHolder>,
}

impl WeightedRandomSelectorFeature {
    pub fn parse(json: &Value) -> Result<WeightedRandomSelectorFeature, String> {
        Ok(WeightedRandomSelectorFeature { features: WeightedList::parse(json.get("features").ok_or("missing features")?, PlacedFeatureHolder::parse)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        match self.features.get_random(random) {
            Some(feature) => feature.value().place(level, random, origin),
            None => false,
        }
    }
}
