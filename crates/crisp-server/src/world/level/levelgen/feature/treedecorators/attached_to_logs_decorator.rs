use crisp_core::Direction;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::tree_decorator::Context;
use crate::util;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;

pub struct AttachedToLogsDecorator {
    probability: f32,
    block_provider: BlockStateProviderHolder,
    directions: Vec<Direction>,
}

pub fn parse_directions(json: &Value) -> Result<Vec<Direction>, String> {
    json.as_array()
        .ok_or("directions is a list")?
        .iter()
        .map(|d| d.as_str().and_then(Direction::by_name).ok_or_else(|| format!("unknown direction {d}")))
        .collect()
}

impl AttachedToLogsDecorator {
    pub fn parse(json: &Value) -> Result<AttachedToLogsDecorator, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(AttachedToLogsDecorator {
            probability: float(field("probability")?)?,
            block_provider: BlockStateProviderHolder::parse(field("block_provider")?)?,
            directions: parse_directions(field("directions")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, context: &mut Context<R>) {
        let mut logs = context.logs.clone();
        util::shuffle(&mut logs, context.random);
        for logs_pos in logs {
            let direction = self.directions[context.random.next_int_bounded(self.directions.len() as i32) as usize];
            let placement_pos = logs_pos.relative(direction);
            if context.random.next_float() <= self.probability && context.is_air(placement_pos) {
                let state = self.block_provider.value().get_state(context.level, context.random, placement_pos);
                context.set_block(placement_pos, state);
            }
        }
    }
}
