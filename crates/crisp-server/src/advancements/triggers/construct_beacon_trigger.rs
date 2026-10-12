use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct ConstructBeaconTrigger(pub CriterionTrigger);

impl ConstructBeaconTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, levels: i32) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::ConstructBeacon(t) = instance else { return false };
            t.matches(levels)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    level: Ints,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance { player: simple_criterion_trigger::decode_player(tag)?, level: codec::decode_or(tag, "level", Ints::ANY, Ints::decode)? })
    }

    pub fn matches(&self, levels: i32) -> bool {
        self.level.matches(levels)
    }
}
