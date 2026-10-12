use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::{self, double_plant_block, mossy_carpet_block};

pub struct SimpleBlockFeature {
    to_place: BlockStateProviderHolder,
    schedule_tick: bool,
}

impl SimpleBlockFeature {
    pub fn parse(json: &Value) -> Result<SimpleBlockFeature, String> {
        Ok(SimpleBlockFeature {
            to_place: BlockStateProviderHolder::parse(json.get("to_place").ok_or("missing to_place")?)?,
            schedule_tick: json.get("schedule_tick").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let Some(state_to_place) = self.to_place.value().get_optional_state(level, random, origin) else {
            return false;
        };
        if !block::of(state_to_place).can_survive(state_to_place, level, origin) {
            return false;
        }
        let block = state_to_place.block();
        if block.is_instance_of("DoublePlantBlock") {
            let above_state = level.get_block_state(origin.above());
            if !above_state.is_air() && (state_to_place.fluid_state() != above_state.fluid_state() || !above_state.can_be_replaced()) {
                return false;
            }
            double_plant_block::place_at(level, state_to_place, origin, 2);
        } else if block.is_instance_of("MossyCarpetBlock") {
            mossy_carpet_block::place_at(level, origin, 2);
        } else {
            level.set_block(origin, state_to_place, 2);
        }
        if self.schedule_tick {
            let block = level.get_block_state(origin).block();
            level.schedule_tick(origin, block, 1);
        }
        true
    }
}
