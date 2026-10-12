use crisp_core::BlockPos;
use crisp_data::{Fluid, FluidState};
use serde_json::Value;

use crate::server::level::legacy_block;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockSet, resource};

pub struct SpringFeature {
    state: FluidState,
    requires_block_below: bool,
    rock_count: i32,
    hole_count: i32,
    valid_blocks: BlockSet,
}

fn parse_fluid_state(json: &Value) -> Result<FluidState, String> {
    let id = json.get("id").and_then(Value::as_str).ok_or("fluid state without id")?;
    let falling = json.get("properties").and_then(|p| p.get("falling")).and_then(Value::as_str) == Some("true");
    let fluid = match resource(id).as_str() {
        "minecraft:water" => Fluid::Water,
        "minecraft:lava" => Fluid::Lava,
        other => return Err(format!("unsupported spring fluid {other}")),
    };
    Ok(FluidState { fluid, amount: 8, falling })
}

impl SpringFeature {
    pub fn parse(json: &Value) -> Result<SpringFeature, String> {
        let int = |key: &str, default: i32| json.get(key).and_then(Value::as_i64).map_or(default, |v| v as i32);
        Ok(SpringFeature {
            state: parse_fluid_state(json.get("state").ok_or("missing state")?)?,
            requires_block_below: json.get("requires_block_below").and_then(Value::as_bool).unwrap_or(true),
            rock_count: int("rock_count", 4),
            hole_count: int("hole_count", 1),
            valid_blocks: BlockSet::parse(json.get("valid_blocks").ok_or("missing valid_blocks")?)?,
        })
    }

    fn is_valid(&self, level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
        self.valid_blocks.contains(level.get_block_state(pos).block())
    }

    pub fn place(&self, level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
        if !self.is_valid(level, origin.above()) {
            return false;
        }
        if self.requires_block_below && !self.is_valid(level, origin.below()) {
            return false;
        }
        let current_state = level.get_block_state(origin);
        if !current_state.is_air() && !self.valid_blocks.contains(current_state.block()) {
            return false;
        }
        let neighbours = [origin.west(), origin.east(), origin.north(), origin.south(), origin.below()];
        let rock_count = neighbours.iter().filter(|&&pos| self.is_valid(level, pos)).count() as i32;
        let hole_count = neighbours.iter().filter(|&&pos| level.get_block_state(pos).is_air()).count() as i32;
        if rock_count == self.rock_count && hole_count == self.hole_count {
            level.set_block(origin, legacy_block(self.state), 2);
            level.schedule_fluid_tick(origin, self.state.fluid, 0);
            return true;
        }
        false
    }
}
