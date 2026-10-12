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
pub struct CuredZombieVillagerTrigger(pub CriterionTrigger);

impl CuredZombieVillagerTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, zombie: EntityId, villager: EntityId) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::CuredZombieVillager(t) = instance else { return false };
            t.matches(level, player, zombie, villager)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    zombie: Option<Holder<LootItemCondition>>,
    villager: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            zombie: simple_criterion_trigger::decode_condition(tag, "zombie")?,
            villager: simple_criterion_trigger::decode_condition(tag, "villager")?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, zombie: EntityId, villager: EntityId) -> bool {
        simple_criterion_trigger::matches_entity(level, player, self.zombie.as_ref(), zombie) && simple_criterion_trigger::matches_entity(level, player, self.villager.as_ref(), villager)
    }
}
