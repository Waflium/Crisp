use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;
use serde_json::Value;

use super::block_replacement::BlockReplacement;
use crate::util::valueproviders::float;

pub struct AbstractOreFeature {
    pub target_states: Vec<BlockReplacement>,
    pub size: i32,
    pub discard_chance_on_air_exposure: f32,
}

impl AbstractOreFeature {
    pub fn parse(json: &Value) -> Result<AbstractOreFeature, String> {
        Ok(AbstractOreFeature {
            target_states: json.get("targets").and_then(Value::as_array).ok_or("missing targets")?.iter().map(BlockReplacement::parse).collect::<Result<_, _>>()?,
            size: json.get("size").and_then(Value::as_i64).ok_or("missing size")? as i32,
            discard_chance_on_air_exposure: float(json.get("discard_chance_on_air_exposure").ok_or("missing discard_chance_on_air_exposure")?)?,
        })
    }

    pub fn can_place_ore(&self, state: BlockState, block_getter: impl Fn(BlockPos) -> BlockState, random: &mut impl RandomSource, target_state: &BlockReplacement, ore_pos: BlockPos) -> bool {
        if !target_state.target.test(state, ore_pos, random) {
            return false;
        }
        should_skip_air_check(random, self.discard_chance_on_air_exposure) || !is_adjacent_to_air(block_getter, ore_pos)
    }
}

pub fn is_adjacent_to_air(block_getter: impl Fn(BlockPos) -> BlockState, pos: BlockPos) -> bool {
    check_neighbors(block_getter, pos, BlockState::is_air)
}

pub fn check_neighbors(block_getter: impl Fn(BlockPos) -> BlockState, pos: BlockPos, predicate: impl Fn(BlockState) -> bool) -> bool {
    Direction::VALUES.into_iter().any(|direction| predicate(block_getter(pos.relative(direction))))
}

fn should_skip_air_check(random: &mut impl RandomSource, discard_chance_on_air_exposure: f32) -> bool {
    if discard_chance_on_air_exposure <= 0.0 {
        true
    } else if discard_chance_on_air_exposure >= 1.0 {
        false
    } else {
        random.next_float() >= discard_chance_on_air_exposure
    }
}
