use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct CoralClawFeature {
    feature: PlacedFeatureHolder,
}

impl CoralClawFeature {
    pub fn parse(json: &Value) -> Result<CoralClawFeature, String> {
        Ok(CoralClawFeature { feature: PlacedFeatureHolder::parse(json.get("feature").ok_or("missing feature")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        if !self.feature.value().place(level, random, origin) {
            return false;
        }
        let claw_direction = Direction::get_random_horizontal_direction(random);
        let n_branches = random.next_int_bounded(2) + 2;
        let mut possible_directions = [claw_direction, claw_direction.clockwise(), claw_direction.counter_clockwise()];
        util::shuffle(&mut possible_directions, random);
        for &branch_direction in &possible_directions[..n_branches as usize] {
            let mut mut_pos = origin;
            let sideway_length = random.next_int_bounded(2) + 1;
            mut_pos = mut_pos.relative(branch_direction);
            let (segment_direction, inway_lenth) = if branch_direction == claw_direction {
                (claw_direction, random.next_int_bounded(3) + 2)
            } else {
                mut_pos = mut_pos.above();
                let segment_possible_directions = [branch_direction, Direction::Up];
                let segment_direction = segment_possible_directions[random.next_int_bounded(2) as usize];
                (segment_direction, random.next_int_bounded(3) + 3)
            };
            let mut i = 0;
            while i < sideway_length && self.feature.value().place(level, random, mut_pos) {
                mut_pos = mut_pos.relative(segment_direction);
                i += 1;
            }
            mut_pos = mut_pos.relative(segment_direction.opposite()).above();
            for _ in 0..inway_lenth {
                mut_pos = mut_pos.relative(claw_direction);
                if !self.feature.value().place(level, random, mut_pos) {
                    break;
                }
                if random.next_float() < 0.25 {
                    mut_pos = mut_pos.above();
                }
            }
        }
        true
    }
}
