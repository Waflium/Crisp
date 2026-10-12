use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct ItemDurabilityTrigger(pub CriterionTrigger);

impl ItemDurabilityTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, item_stack: &ItemStack, new_durability: i32) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::ItemDurability(t) = instance else { return false };
            t.matches(item_stack, new_durability)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    item: Option<ItemPredicate>,
    durability: Ints,
    delta: Ints,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            item: codec::decode_optional(tag, "item", ItemPredicate::decode)?,
            durability: codec::decode_or(tag, "durability", Ints::ANY, Ints::decode)?,
            delta: codec::decode_or(tag, "delta", Ints::ANY, Ints::decode)?,
        })
    }

    pub fn matches(&self, item_stack: &ItemStack, new_durability: i32) -> bool {
        if self.item.as_ref().is_some_and(|item| !item.test(item_stack)) {
            return false;
        }
        if !self.durability.matches(item_stack.max_damage() - new_durability) {
            return false;
        }
        self.delta.matches(item_stack.damage_value() - new_durability)
    }
}
