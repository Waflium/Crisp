use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::placement::placed_feature::{PlacedFeatureHolder, parse_placed_feature_set};

pub struct SimpleRandomSelectorFeature {
    features: Vec<PlacedFeatureHolder>,
}

impl SimpleRandomSelectorFeature {
    pub fn parse(json: &Value) -> Result<SimpleRandomSelectorFeature, String> {
        Ok(SimpleRandomSelectorFeature { features: parse_placed_feature_set(json.get("features").ok_or("missing features")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let index = random.next_int_bounded(self.features.len() as i32);
        self.features[index as usize].value().place(level, random, origin)
    }
}
