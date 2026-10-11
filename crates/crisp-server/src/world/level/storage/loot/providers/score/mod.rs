use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::scores::score_holder::ScoreHolder;

pub enum ScoreboardNameProvider {
    Fixed(String),
    Context(EntityTarget),
}

impl ScoreboardNameProvider {
    pub fn decode(tag: &Nbt) -> DataResult<ScoreboardNameProvider> {
        match codec::either(tag, EntityTarget::decode, |t| {
            let ty = codec::decode_field(t, "type", codec::identifier)?;
            match ty.as_str() {
                "minecraft:fixed" => Ok(ScoreboardNameProvider::Fixed(codec::decode_field(t, "name", |n| Ok(codec::string(n)?.to_string()))?)),
                "minecraft:context" => Ok(ScoreboardNameProvider::Context(codec::decode_field(t, "target", EntityTarget::decode)?)),
                other => Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_score_provider_type]: {other}")),
            }
        })? {
            Ok(target) => Ok(ScoreboardNameProvider::Context(target)),
            Err(provider) => Ok(provider),
        }
    }

    pub fn get_score_holder(&self, context: &LootContext) -> Option<ScoreHolder> {
        match self {
            ScoreboardNameProvider::Fixed(name) => Some(ScoreHolder::for_name_only(name.clone())),
            ScoreboardNameProvider::Context(target) => target.get(&context.params).map(|entity| ScoreHolder::for_entity(context.params.level, entity)),
        }
    }

    pub fn score_value(&self, context: &LootContext, score: &str) -> Option<i32> {
        let score_holder = self.get_score_holder(context)?;
        let scoreboard = &context.params.level.scoreboard;
        let objective = scoreboard.get_objective(score)?;
        scoreboard.get_player_score_info(score_holder.scoreboard_name(), objective.id()).map(|info| info.value)
    }
}
