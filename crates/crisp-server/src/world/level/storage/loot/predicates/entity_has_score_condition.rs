use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::int_range_predicate::IntRangePredicate;
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::scores::score_holder::ScoreHolder;

pub struct EntityHasScoreCondition {
    scores: Vec<(String, IntRangePredicate)>,
    entity_target: EntityTarget,
}

impl EntityHasScoreCondition {
    pub fn decode(tag: &Nbt) -> DataResult<EntityHasScoreCondition> {
        Ok(EntityHasScoreCondition {
            scores: codec::decode_field(tag, "scores", |t| codec::unbounded_map(t, |k| Ok(k.to_string()), IntRangePredicate::decode))?,
            entity_target: codec::decode_field(tag, "entity", EntityTarget::decode)?,
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(entity) = self.entity_target.get(&context.params) else { return false };
        let holder = ScoreHolder::for_entity(context.params.level, entity);
        self.scores.iter().all(|(objective_name, range)| self.has_score(context, &holder, objective_name, range))
    }

    fn has_score(&self, context: &mut LootContext, holder: &ScoreHolder, objective_name: &str, range: &IntRangePredicate) -> bool {
        let value = {
            let scoreboard = context.params.level.scoreboard.borrow();
            let Some(objective) = scoreboard.get_objective(objective_name) else { return false };
            let Some(value) = scoreboard.get_player_score_info(holder.scoreboard_name(), objective.id()).map(|score| score.value) else { return false };
            value
        };
        range.test(context, value)
    }
}
