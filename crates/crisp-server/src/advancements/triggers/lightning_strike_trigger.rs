use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct LightningStrikeTrigger(pub CriterionTrigger);

impl LightningStrikeTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, lightning: EntityId, entities_around: &[EntityId]) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::LightningStrike(t) = instance else { return false };
            t.matches(level, player, lightning, entities_around)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    lightning: Option<Holder<LootItemCondition>>,
    bystander: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            lightning: simple_criterion_trigger::decode_condition(tag, "lightning")?,
            bystander: simple_criterion_trigger::decode_condition(tag, "bystander")?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, bolt: EntityId, entities_around: &[EntityId]) -> bool {
        if !simple_criterion_trigger::matches_entity(level, player, self.lightning.as_ref(), bolt) {
            return false;
        }
        match &self.bystander {
            None => true,
            Some(bystander) => entities_around.iter().any(|&entity| simple_criterion_trigger::matches_entity(level, player, Some(bystander), entity)),
        }
    }
}
