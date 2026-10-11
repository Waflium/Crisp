use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct SimpleRandomSelectorFeature {
    features: Vec<PlacedFeatureHolder>,
}

impl SimpleRandomSelectorFeature {
    pub fn parse(json: &Value) -> Result<SimpleRandomSelectorFeature, String> {
        let features = match json.get("features").ok_or("missing features")? {
            Value::Array(list) => list.iter().map(PlacedFeatureHolder::parse).collect::<Result<Vec<_>, _>>()?,
            other => vec![PlacedFeatureHolder::parse(other)?],
        };
        if features.is_empty() {
            return Err("features must not be empty".into());
        }
        Ok(SimpleRandomSelectorFeature { features })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let index = random.next_int_bounded(self.features.len() as i32);
        self.features[index as usize].value().place(level, random, origin)
    }
}
