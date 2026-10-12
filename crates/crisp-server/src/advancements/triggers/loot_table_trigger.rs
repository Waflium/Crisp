use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct LootTableTrigger(pub CriterionTrigger);

impl LootTableTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, loot_table: &str) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::LootTable(t) = instance else { return false };
            t.matches(loot_table)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    loot_table: Vec<Holder<LootTable>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance { player: simple_criterion_trigger::decode_player(tag)?, loot_table: codec::decode_field(tag, "loot_tables", LootTable::decode_list)? })
    }

    pub fn matches(&self, loot_table: &str) -> bool {
        self.loot_table.iter().any(|table| table.unwrap_key() == Some(loot_table))
    }
}
