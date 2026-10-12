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
pub struct EnchantedItemTrigger(pub CriterionTrigger);

impl EnchantedItemTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, item_stack: &ItemStack, levels: i32) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::EnchantedItem(t) = instance else { return false };
            t.matches(item_stack, levels)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    item: Option<ItemPredicate>,
    levels: Ints,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            item: codec::decode_optional(tag, "item", ItemPredicate::decode)?,
            levels: codec::decode_or(tag, "levels", Ints::ANY, Ints::decode)?,
        })
    }

    pub fn matches(&self, item_stack: &ItemStack, levels: i32) -> bool {
        if self.item.as_ref().is_some_and(|item| !item.test(item_stack)) {
            return false;
        }
        self.levels.matches(levels)
    }
}
