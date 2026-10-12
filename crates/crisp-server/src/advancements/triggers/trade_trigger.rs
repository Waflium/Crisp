use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct TradeTrigger(pub CriterionTrigger);

impl TradeTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, villager: EntityId, item_stack: &ItemStack) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::Trade(t) = instance else { return false };
            t.matches(level, player, villager, item_stack)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    villager: Option<Holder<LootItemCondition>>,
    item: Option<ItemPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            villager: simple_criterion_trigger::decode_condition(tag, "villager")?,
            item: codec::decode_optional(tag, "item", ItemPredicate::decode)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, villager: EntityId, item_stack: &ItemStack) -> bool {
        if !simple_criterion_trigger::matches_entity(level, player, self.villager.as_ref(), villager) {
            return false;
        }
        self.item.as_ref().is_none_or(|item| item.test(item_stack))
    }
}
