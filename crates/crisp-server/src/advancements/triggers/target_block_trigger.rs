use crisp_core::Vec3;
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
pub struct TargetBlockTrigger(pub CriterionTrigger);

impl TargetBlockTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, projectile: EntityId, hit_position: Vec3, signal_strength: i32) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::TargetBlock(t) = instance else { return false };
            t.matches(level, player, projectile, hit_position, signal_strength)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    signal_strength: Ints,
    projectile: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            signal_strength: codec::decode_or(tag, "signal_strength", Ints::ANY, Ints::decode)?,
            projectile: simple_criterion_trigger::decode_condition(tag, "projectile")?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, projectile: EntityId, _hit_position: Vec3, signal_strength: i32) -> bool {
        if !self.signal_strength.matches(signal_strength) {
            return false;
        }
        simple_criterion_trigger::matches_entity(level, player, self.projectile.as_ref(), projectile)
    }
}
