use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::damage_predicate::DamagePredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct EntityHurtPlayerTrigger(pub CriterionTrigger);

impl EntityHurtPlayerTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, source: &DamageSource, original_damage: f32, actual_damage: f32, blocked: bool) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::EntityHurtPlayer(t) = instance else { return false };
            t.matches(level, player, source, original_damage, actual_damage, blocked)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    damage: Option<DamagePredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance { player: simple_criterion_trigger::decode_player(tag)?, damage: codec::decode_optional(tag, "damage", DamagePredicate::decode)? })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, source: &DamageSource, original_damage: f32, actual_damage: f32, blocked: bool) -> bool {
        self.damage.as_ref().is_none_or(|damage| damage.matches(level, player, source, original_damage, actual_damage, blocked))
    }
}
