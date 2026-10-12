use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::sculk_behaviour::SculkBehaviour;
use crate::world::level::block::sculk_spreader::SculkSpreader;

pub struct SculkPatchFeature {
    charge_count: i32,
    amount_per_charge: i32,
    spread_attempts: i32,
    growth_rounds: i32,
    spread_rounds: i32,
}

impl SculkPatchFeature {
    pub fn parse(json: &Value) -> Result<SculkPatchFeature, String> {
        let int = |key: &str| json.get(key).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| format!("missing {key}"));
        Ok(SculkPatchFeature {
            charge_count: int("charge_count")?,
            amount_per_charge: int("amount_per_charge")?,
            spread_attempts: int("spread_attempts")?,
            growth_rounds: int("growth_rounds")?,
            spread_rounds: int("spread_rounds")?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        if !can_spread_from(level, origin) {
            return false;
        }
        let mut spreader = SculkSpreader::create_world_gen_spreader();
        let total_rounds = self.spread_rounds + self.growth_rounds;
        for round in 0..total_rounds {
            for _ in 0..self.charge_count {
                spreader.add_cursors(origin, self.amount_per_charge);
            }
            let spread_veins = round < self.spread_rounds;
            for _ in 0..self.spread_attempts {
                spreader.update_cursors(level, origin, random, spread_veins);
            }
            spreader.clear();
        }
        true
    }
}

fn can_spread_from(level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
    let start = level.get_block_state(origin);
    if SculkBehaviour::of(start.block()).is_some() {
        return true;
    }
    if !start.is_air() && (!start.is_block(blocks::WATER) || !start.fluid_state().is_source()) {
        return false;
    }
    Direction::VALUES.into_iter().any(|direction| level.get_block_state(origin.relative(direction)).is_collision_shape_full_block())
}
