use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use serde_json::Value;

use super::abstract_ore_feature::AbstractOreFeature;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct ScatteredOreFeature {
    pub ore: AbstractOreFeature,
}

impl ScatteredOreFeature {
    pub fn parse(json: &Value) -> Result<ScatteredOreFeature, String> {
        Ok(ScatteredOreFeature { ore: AbstractOreFeature::parse(json)? })
    }

    pub fn place(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, origin: BlockPos) -> bool {
        let number_of_tries = random.next_int_bounded(self.ore.size + 1);
        for i in 0..number_of_tries {
            let target_pos = offset_target_pos(random, origin, i.min(7));
            let block_state = level.get_block_state(target_pos);
            for target_state in &self.ore.target_states {
                if self.ore.can_place_ore(block_state, |pos| level.get_block_state(pos), random, target_state, target_pos) {
                    level.set_block(target_pos, target_state.state, UPDATE_CLIENTS);
                    break;
                }
            }
        }
        true
    }
}

fn offset_target_pos(random: &mut impl RandomSource, origin: BlockPos, max_dist_from_origin_for_this_try: i32) -> BlockPos {
    let xd = random_placement_in_one_axis_relative_to_origin(random, max_dist_from_origin_for_this_try);
    let yd = random_placement_in_one_axis_relative_to_origin(random, max_dist_from_origin_for_this_try);
    let zd = random_placement_in_one_axis_relative_to_origin(random, max_dist_from_origin_for_this_try);
    origin.offset(xd, yd, zd)
}

fn random_placement_in_one_axis_relative_to_origin(random: &mut impl RandomSource, max_distance_from_origin: i32) -> i32 {
    mth::java_round_f32((random.next_float() - random.next_float()) * max_distance_from_origin as f32)
}
