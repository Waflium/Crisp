use crisp_data::BlockState;
use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::advancements::predicates::state_properties_predicate::StatePropertiesPredicate;
use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct BeeNestDestroyedTrigger(pub CriterionTrigger);

impl BeeNestDestroyedTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, state: BlockState, item_stack: &ItemStack, num_bees_inside: i32) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::BeeNestDestroyed(t) = instance else { return false };
            t.matches(state, item_stack, num_bees_inside)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    block: Option<HolderSet>,
    state: Option<StatePropertiesPredicate>,
    item: Option<ItemPredicate>,
    bees_inside: Ints,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            block: codec::decode_optional(tag, "blocks", |t| HolderSet::decode("minecraft:block", t))?,
            state: codec::decode_optional(tag, "state", StatePropertiesPredicate::decode)?,
            item: codec::decode_optional(tag, "item", ItemPredicate::decode)?,
            bees_inside: codec::decode_or(tag, "num_bees_inside", Ints::ANY, Ints::decode)?,
        })
    }

    pub fn matches(&self, state: BlockState, item_stack: &ItemStack, num_bees_inside: i32) -> bool {
        if self.block.as_ref().is_some_and(|block| !block.contains(state.block().0 as i32)) {
            return false;
        }
        if self.state.as_ref().is_some_and(|predicate| !predicate.matches_block(state)) {
            return false;
        }
        if self.item.as_ref().is_some_and(|item| !item.test(item_stack)) {
            return false;
        }
        self.bees_inside.matches(num_bees_inside)
    }
}
