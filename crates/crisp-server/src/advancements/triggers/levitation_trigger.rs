use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::distance_predicate::DistancePredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct LevitationTrigger(pub CriterionTrigger);

impl LevitationTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, start: Vec3, duration: i32) {
        let position = level.entity(player).position();
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::Levitation(t) = instance else { return false };
            t.matches(position, start, duration)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    distance: Option<DistancePredicate>,
    duration: Ints,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            distance: codec::decode_optional(tag, "distance", DistancePredicate::decode)?,
            duration: codec::decode_or(tag, "duration", Ints::ANY, Ints::decode)?,
        })
    }

    pub fn matches(&self, player: Vec3, start: Vec3, duration: i32) -> bool {
        if self.distance.as_ref().is_some_and(|distance| !distance.matches(start.x, start.y, start.z, player.x, player.y, player.z)) {
            return false;
        }
        self.duration.matches(duration)
    }
}
