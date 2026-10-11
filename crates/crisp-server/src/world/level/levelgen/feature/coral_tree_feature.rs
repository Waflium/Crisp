use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct CoralTreeFeature {
    feature: PlacedFeatureHolder,
}

impl CoralTreeFeature {
    pub fn parse(json: &Value) -> Result<CoralTreeFeature, String> {
        Ok(CoralTreeFeature { feature: PlacedFeatureHolder::parse(json.get("feature").ok_or("missing feature")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let mut mut_pos = origin;
        let trunck_height = random.next_int_bounded(3) + 1;
        for _ in 0..trunck_height {
            if !self.feature.value().place(level, random, mut_pos) {
                return true;
            }
            mut_pos = mut_pos.above();
        }
        let trunck_top_pos = mut_pos;
        let n_branches = random.next_int_bounded(3) + 2;
        let mut directions = Direction::PLANE_HORIZONTAL;
        util::shuffle(&mut directions, random);
        for &branch_direction in &directions[..n_branches as usize] {
            mut_pos = trunck_top_pos.relative(branch_direction);
            let branch_height = random.next_int_bounded(5) + 2;
            let mut segment_length = 0;
            let mut j = 0;
            while j < branch_height && self.feature.value().place(level, random, mut_pos) {
                segment_length += 1;
                mut_pos = mut_pos.above();
                if j == 0 || segment_length >= 2 && random.next_float() < 0.25 {
                    mut_pos = mut_pos.relative(branch_direction);
                    segment_length = 0;
                }
                j += 1;
            }
        }
        true
    }
}
