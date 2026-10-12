use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::damage_source_predicate::DamageSourcePredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct KilledTrigger(pub CriterionTrigger);

impl KilledTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, entity: EntityId, killing_blow: &DamageSource) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::Killed(t) = instance else { return false };
            t.matches(level, player, entity, killing_blow)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    entity: Option<Holder<LootItemCondition>>,
    killing_blow: Option<DamageSourcePredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            entity: simple_criterion_trigger::decode_condition(tag, "entity")?,
            killing_blow: codec::decode_optional(tag, "killing_blow", DamageSourcePredicate::decode)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, entity: EntityId, killing_blow: &DamageSource) -> bool {
        if let Some(predicate) = &self.killing_blow {
            let position = level.entity(player).position();
            if !predicate.matches(level, Some(position), killing_blow) {
                return false;
            }
        }
        simple_criterion_trigger::matches_entity(level, player, self.entity.as_ref(), entity)
    }
}
