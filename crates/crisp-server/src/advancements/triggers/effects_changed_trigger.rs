use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::mob_effects_predicate::MobEffectsPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct EffectsChangedTrigger(pub CriterionTrigger);

impl EffectsChangedTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, source: Option<EntityId>) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::EffectsChanged(t) = instance else { return false };
            t.matches(level, player, source)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    effects: Option<MobEffectsPredicate>,
    source: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            effects: codec::decode_optional(tag, "effects", MobEffectsPredicate::decode)?,
            source: simple_criterion_trigger::decode_condition(tag, "source")?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, source: Option<EntityId>) -> bool {
        if self.effects.as_ref().is_some_and(|effects| !effects.matches(&level.living(player).active_effects)) {
            return false;
        }
        match &self.source {
            None => true,
            Some(predicate) => source.is_some_and(|source| simple_criterion_trigger::matches_entity(level, player, Some(predicate), source)),
        }
    }
}
