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
pub struct PlayerInteractTrigger(pub CriterionTrigger);

impl PlayerInteractTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, item_stack: &ItemStack, interacted_with: EntityId) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::PlayerInteract(t) = instance else { return false };
            t.matches(level, player, item_stack, interacted_with)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    item: Option<ItemPredicate>,
    entity: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            item: codec::decode_optional(tag, "item", ItemPredicate::decode)?,
            entity: simple_criterion_trigger::decode_condition(tag, "entity")?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, item_stack: &ItemStack, interacted_with: EntityId) -> bool {
        if self.item.as_ref().is_some_and(|item| !item.test(item_stack)) {
            return false;
        }
        simple_criterion_trigger::matches_entity(level, player, self.entity.as_ref(), interacted_with)
    }
}
