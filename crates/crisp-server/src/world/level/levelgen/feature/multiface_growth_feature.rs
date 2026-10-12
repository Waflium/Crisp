use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, blocks};
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util;
use crate::util::valueproviders::float;
use crate::world::level::block::multiface_block;
use crate::world::level::block::multiface_spreader::{self, MultifaceSpreader, SpreadRandom};
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockSet, resource};

pub struct MultifaceGrowthFeature {
    place_block: Block,
    search_range: i32,
    can_place_on_floor: bool,
    can_place_on_ceiling: bool,
    can_place_on_wall: bool,
    chance_of_spreading: f32,
    can_be_placed_on: BlockSet,
}

impl MultifaceGrowthFeature {
    pub fn parse(json: &Value) -> Result<MultifaceGrowthFeature, String> {
        let flag = |key: &str| json.get(key).and_then(Value::as_bool).unwrap_or(false);
        let block = json.get("block").and_then(Value::as_str).ok_or("missing block")?;
        Ok(MultifaceGrowthFeature {
            place_block: Block::by_name(&resource(block)).ok_or_else(|| format!("unknown block {block}"))?,
            search_range: json.get("search_range").and_then(Value::as_i64).map_or(10, |v| v as i32),
            can_place_on_floor: flag("can_place_on_floor"),
            can_place_on_ceiling: flag("can_place_on_ceiling"),
            can_place_on_wall: flag("can_place_on_wall"),
            chance_of_spreading: json.get("chance_of_spreading").map(float).transpose()?.unwrap_or(0.5),
            can_be_placed_on: BlockSet::parse(json.get("can_be_placed_on").ok_or("missing can_be_placed_on")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        if !is_air_or_water(level.get_block_state(origin)) {
            return false;
        }
        let Some(spreader) = multiface_spreader::spreader_for(self.place_block) else {
            return false;
        };
        let directions = self.get_shuffled_directions(random);
        let origin_state = level.get_block_state(origin);
        if self.place_growth_if_possible(&spreader, level, origin, origin_state, random, &directions) {
            return true;
        }
        for &search_direction in &directions {
            let placement_directions = self.get_shuffled_directions_except(random, search_direction.opposite());
            for _ in 0..self.search_range {
                let pos = origin.relative(search_direction);
                let state = level.get_block_state(pos);
                if !is_air_or_water(state) && !state.is_block(self.place_block) {
                    break;
                }
                if self.place_growth_if_possible(&spreader, level, pos, state, random, &placement_directions) {
                    return true;
                }
            }
        }
        false
    }

    fn place_growth_if_possible<R: RandomSource>(
        &self,
        spreader: &MultifaceSpreader,
        level: &mut impl WorldGenLevel,
        pos: BlockPos,
        old_state: BlockState,
        random: &mut R,
        placement_directions: &[Direction],
    ) -> bool {
        for &placement_direction in placement_directions {
            let neighbour_state = level.get_block_state(pos.relative(placement_direction));
            if self.can_be_placed_on.contains(neighbour_state.block()) {
                let Some(new_state) = multiface_block::get_state_for_placement(self.place_block, old_state, level, pos, placement_direction) else {
                    return false;
                };
                level.set_block_and_update(pos, new_state);
                level.mark_pos_for_post_processing(pos);
                if random.next_float() < self.chance_of_spreading {
                    spreader.spread_from_face_toward_random_direction(new_state, level, pos, placement_direction, &mut SpreadRandom::Own(random), true);
                }
                return true;
            }
        }
        false
    }

    fn valid_directions(&self) -> Vec<Direction> {
        let mut valid_directions = Vec::with_capacity(6);
        if self.can_place_on_ceiling {
            valid_directions.push(Direction::Up);
        }
        if self.can_place_on_floor {
            valid_directions.push(Direction::Down);
        }
        if self.can_place_on_wall {
            valid_directions.extend(Direction::PLANE_HORIZONTAL);
        }
        valid_directions
    }

    fn get_shuffled_directions_except(&self, random: &mut impl RandomSource, exclude_direction: Direction) -> Vec<Direction> {
        let mut directions: Vec<Direction> = self.valid_directions().into_iter().filter(|&direction| direction != exclude_direction).collect();
        util::shuffle(&mut directions, random);
        directions
    }

    fn get_shuffled_directions(&self, random: &mut impl RandomSource) -> Vec<Direction> {
        let mut directions = self.valid_directions();
        util::shuffle(&mut directions, random);
        directions
    }
}

fn is_air_or_water(state: BlockState) -> bool {
    state.is_air() || state.is_block(blocks::WATER)
}
