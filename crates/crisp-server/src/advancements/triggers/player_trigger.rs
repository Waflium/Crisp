use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct PlayerTrigger(pub CriterionTrigger);

impl PlayerTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, _| true);
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance { player: simple_criterion_trigger::decode_player(tag)? })
    }
}
