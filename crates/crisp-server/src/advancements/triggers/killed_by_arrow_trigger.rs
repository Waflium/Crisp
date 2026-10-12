use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct KilledByArrowTrigger(pub CriterionTrigger);

impl KilledByArrowTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, victims: &[EntityId], fired_by_weapon: Option<&ItemStack>) {
        let mut entity_types: Vec<i32> = Vec::new();
        for &victim in victims {
            let entity_type = level.entity(victim).entity_type;
            if !entity_types.contains(&entity_type) {
                entity_types.push(entity_type);
            }
        }
        let unique_entity_types = entity_types.len() as i32;
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::KilledByArrow(t) = instance else { return false };
            t.matches(level, player, victims, unique_entity_types, fired_by_weapon)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    victims: Vec<Holder<LootItemCondition>>,
    unique_entity_types: Ints,
    fired_from_weapon: Option<ItemPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            victims: simple_criterion_trigger::decode_conditions(tag, "victims")?,
            unique_entity_types: codec::decode_or(tag, "unique_entity_types", Ints::ANY, Ints::decode)?,
            fired_from_weapon: codec::decode_optional(tag, "fired_from_weapon", ItemPredicate::decode)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, victims: &[EntityId], unique_entity_types: i32, fired_from_weapon: Option<&ItemStack>) -> bool {
        if let Some(predicate) = &self.fired_from_weapon
            && !fired_from_weapon.is_some_and(|weapon| predicate.test(weapon))
        {
            return false;
        }
        if !self.victims.is_empty() {
            let mut victims_copy: Vec<EntityId> = victims.to_vec();
            for predicate in &self.victims {
                match victims_copy.iter().position(|&entity| simple_criterion_trigger::matches_entity(level, player, Some(predicate), entity)) {
                    Some(found) => {
                        victims_copy.remove(found);
                    }
                    None => return false,
                }
            }
        }
        self.unique_entity_types.matches(unique_entity_types)
    }
}
