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
pub struct BredAnimalsTrigger(pub CriterionTrigger);

impl BredAnimalsTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, parent: EntityId, partner: EntityId, child: Option<EntityId>) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::BredAnimals(t) = instance else { return false };
            t.matches(level, player, parent, partner, child)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    parent: Option<Holder<LootItemCondition>>,
    partner: Option<Holder<LootItemCondition>>,
    child: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            parent: simple_criterion_trigger::decode_condition(tag, "parent")?,
            partner: simple_criterion_trigger::decode_condition(tag, "partner")?,
            child: simple_criterion_trigger::decode_condition(tag, "child")?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, parent: EntityId, partner: EntityId, child: Option<EntityId>) -> bool {
        if self.child.is_some() {
            let Some(child) = child else { return false };
            if !simple_criterion_trigger::matches_entity(level, player, self.child.as_ref(), child) {
                return false;
            }
        }
        let matches = |level: &mut ServerLevel, predicate: &Option<Holder<LootItemCondition>>, entity: EntityId| {
            simple_criterion_trigger::matches_entity(level, player, predicate.as_ref(), entity)
        };
        matches(level, &self.parent, parent) && matches(level, &self.partner, partner) || matches(level, &self.parent, partner) && matches(level, &self.partner, parent)
    }
}
