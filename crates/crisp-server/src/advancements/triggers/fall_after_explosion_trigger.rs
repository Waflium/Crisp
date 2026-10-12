use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::distance_predicate::DistancePredicate;
use crate::advancements::predicates::location_predicate::LocationPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct FallAfterExplosionTrigger(pub CriterionTrigger);

impl FallAfterExplosionTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, start_position: Vec3, cause: Option<EntityId>) {
        let player_position = level.entity(player).position();
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::FallAfterExplosion(t) = instance else { return false };
            t.matches(level, player, start_position, player_position, cause)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    start_position: Option<LocationPredicate>,
    distance: Option<DistancePredicate>,
    cause: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            start_position: codec::decode_optional(tag, "start_position", LocationPredicate::decode)?,
            distance: codec::decode_optional(tag, "distance", DistancePredicate::decode)?,
            cause: simple_criterion_trigger::decode_condition(tag, "cause")?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, entered_position: Vec3, player_position: Vec3, cause: Option<EntityId>) -> bool {
        if self.start_position.as_ref().is_some_and(|start| !start.matches(level, entered_position.x, entered_position.y, entered_position.z)) {
            return false;
        }
        if self.distance.as_ref().is_some_and(|distance| {
            !distance.matches(entered_position.x, entered_position.y, entered_position.z, player_position.x, player_position.y, player_position.z)
        }) {
            return false;
        }
        match &self.cause {
            None => true,
            Some(predicate) => cause.is_some_and(|cause| simple_criterion_trigger::matches_entity(level, player, Some(predicate), cause)),
        }
    }
}
