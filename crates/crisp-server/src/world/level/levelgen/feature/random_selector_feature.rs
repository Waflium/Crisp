use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct WeightedPlacedFeature {
    feature: PlacedFeatureHolder,
    chance: f32,
}

impl WeightedPlacedFeature {
    fn parse(json: &Value) -> Result<WeightedPlacedFeature, String> {
        Ok(WeightedPlacedFeature {
            feature: PlacedFeatureHolder::parse(json.get("feature").ok_or("missing feature")?)?,
            chance: float(json.get("chance").ok_or("missing chance")?)?,
        })
    }

    fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        self.feature.value().place(level, random, origin)
    }
}

pub struct RandomSelectorFeature {
    features: Vec<WeightedPlacedFeature>,
    default_feature: PlacedFeatureHolder,
}

impl RandomSelectorFeature {
    pub fn parse(json: &Value) -> Result<RandomSelectorFeature, String> {
        Ok(RandomSelectorFeature {
            features: json.get("features").and_then(Value::as_array).ok_or("missing features")?.iter().map(WeightedPlacedFeature::parse).collect::<Result<_, _>>()?,
            default_feature: PlacedFeatureHolder::parse(json.get("default").ok_or("missing default")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        for feature in &self.features {
            if random.next_float() < feature.chance {
                return feature.place(level, random, origin);
            }
        }
        self.default_feature.value().place(level, random, origin)
    }
}
