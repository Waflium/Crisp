use crisp_data::BlockState;
use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::state_properties_predicate::StatePropertiesPredicate;
use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct SlideDownBlockTrigger(pub CriterionTrigger);

impl SlideDownBlockTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, state: BlockState) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::SlideDownBlock(t) = instance else { return false };
            t.matches(state)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    block: Option<HolderSet>,
    state: Option<StatePropertiesPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            block: codec::decode_optional(tag, "blocks", |t| HolderSet::decode("minecraft:block", t))?,
            state: codec::decode_optional(tag, "state", StatePropertiesPredicate::decode)?,
        })
    }

    pub fn matches(&self, state: BlockState) -> bool {
        if self.block.as_ref().is_some_and(|block| !block.contains(state.block().0 as i32)) {
            return false;
        }
        self.state.as_ref().is_none_or(|predicate| predicate.matches_block(state))
    }
}
