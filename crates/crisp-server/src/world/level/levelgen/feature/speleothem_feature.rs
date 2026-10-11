use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;
use serde_json::Value;

use super::speleothem_utils;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;
use crate::world::level::levelgen::parse_block_state;

pub struct SpeleothemFeature {
    base_block: BlockState,
    pointed_block: BlockState,
    replaceable_blocks: BlockSet,
    chance_of_taller_generation: f32,
    chance_of_directional_spread: f32,
    chance_of_spread_radius2: f32,
    chance_of_spread_radius3: f32,
}

impl SpeleothemFeature {
    pub fn parse(json: &Value) -> Result<SpeleothemFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let chance = |key: &str, default: f32| json.get(key).map(float).transpose().map(|v| v.unwrap_or(default));
        Ok(SpeleothemFeature {
            base_block: parse_block_state(field("base_block")?)?,
            pointed_block: parse_block_state(field("pointed_block")?)?,
            replaceable_blocks: BlockSet::parse(field("replaceable_blocks")?)?,
            chance_of_taller_generation: chance("chance_of_taller_generation", 0.2)?,
            chance_of_directional_spread: chance("chance_of_directional_spread", 0.7)?,
            chance_of_spread_radius2: chance("chance_of_spread_radius2", 0.5)?,
            chance_of_spread_radius3: chance("chance_of_spread_radius3", 0.5)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let Some(tip_direction) = self.get_tip_direction(level, origin, random) else {
            return false;
        };
        let root_pos = origin.relative(tip_direction.opposite());
        self.create_patch_of_base_blocks(level, random, root_pos);
        let height = if random.next_float() < self.chance_of_taller_generation && speleothem_utils::is_empty_or_water(level.get_block_state(origin.relative(tip_direction))) { 2 } else { 1 };
        speleothem_utils::grow_speleothem(level, origin, tip_direction, height, false, self.base_block.block(), self.pointed_block.block(), &self.replaceable_blocks);
        true
    }

    fn get_tip_direction(&self, level: &mut WorldGenRegion, pos: BlockPos, random: &mut impl RandomSource) -> Option<Direction> {
        let can_place_above = speleothem_utils::is_base(level.get_block_state(pos.above()), self.base_block.block(), &self.replaceable_blocks);
        let can_place_below = speleothem_utils::is_base(level.get_block_state(pos.below()), self.base_block.block(), &self.replaceable_blocks);
        if can_place_above && can_place_below {
            Some(if random.next_boolean() { Direction::Down } else { Direction::Up })
        } else if can_place_above {
            Some(Direction::Down)
        } else if can_place_below {
            Some(Direction::Up)
        } else {
            None
        }
    }

    fn create_patch_of_base_blocks(&self, level: &mut WorldGenRegion, random: &mut impl RandomSource, pos: BlockPos) {
        let base = self.base_block.block();
        speleothem_utils::place_base_block_if_possible(level, pos, base, &self.replaceable_blocks);
        for direction in Direction::PLANE_HORIZONTAL {
            if random.next_float() <= self.chance_of_directional_spread {
                let pos1 = pos.relative(direction);
                speleothem_utils::place_base_block_if_possible(level, pos1, base, &self.replaceable_blocks);
                if random.next_float() <= self.chance_of_spread_radius2 {
                    let pos2 = pos1.relative(Direction::get_random(random));
                    speleothem_utils::place_base_block_if_possible(level, pos2, base, &self.replaceable_blocks);
                    if random.next_float() <= self.chance_of_spread_radius3 {
                        let pos3 = pos2.relative(Direction::get_random(random));
                        speleothem_utils::place_base_block_if_possible(level, pos3, base, &self.replaceable_blocks);
                    }
                }
            }
        }
    }
}
