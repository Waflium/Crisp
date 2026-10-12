use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::placement::feature_placer::FeaturePlacer;
use crate::world::level::levelgen::placement::placed_feature::{PlacedFeatureHolder, parse_placed_feature_set};

pub struct OverlayFeature {
    features: Vec<PlacedFeatureHolder>,
}

impl OverlayFeature {
    pub fn parse(json: &Value) -> Result<OverlayFeature, String> {
        Ok(OverlayFeature { features: parse_placed_feature_set(json.get("features").ok_or("missing features")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut placer = FeaturePlacer::default();
        let mut placed_any = false;
        for feature in &self.features {
            placed_any |= placer.place(level, feature.value(), random, origin);
        }
        placed_any
    }
}
