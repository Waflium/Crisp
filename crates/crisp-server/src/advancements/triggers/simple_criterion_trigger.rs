use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::entity::entity_predicate::EntityPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::server::player_advancements::{PlayerAdvancements, TriggerInstanceKey};
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

pub fn trigger(level: &mut ServerLevel, player: EntityId, trigger: CriterionTrigger, mut matcher: impl FnMut(&mut ServerLevel, &'static CriterionTriggerInstance) -> bool) {
    let Some(listeners_for_type) = level.player(player).advancements.get_trigger_map_for_type(trigger) else { return };
    if listeners_for_type.is_empty() {
        return;
    }
    let listeners: Vec<_> = listeners_for_type.iter().collect();
    let mut matched_conditions: Vec<TriggerInstanceKey> = Vec::new();
    for listener in listeners {
        if matcher(level, listener.instance) {
            let player_matches = match listener.instance.player() {
                None => true,
                Some(predicate) => predicate.value().test(&mut EntityPredicate::create_context(level, player, player)),
            };
            if player_matches {
                matched_conditions.push(listener.key);
            }
        }
    }
    for criterion in matched_conditions {
        PlayerAdvancements::award(level, player, criterion.advancement, criterion.criterion);
    }
}

pub fn decode_player(tag: &Nbt) -> DataResult<Option<Holder<LootItemCondition>>> {
    codec::map(tag)?;
    codec::decode_optional(tag, "player", LootItemCondition::decode_holder)
}

pub fn decode_condition(tag: &Nbt, name: &str) -> DataResult<Option<Holder<LootItemCondition>>> {
    codec::decode_optional(tag, name, LootItemCondition::decode_holder)
}

pub fn decode_conditions(tag: &Nbt, name: &str) -> DataResult<Vec<Holder<LootItemCondition>>> {
    codec::decode_or(tag, name, Vec::new(), |t| codec::list_of(t, LootItemCondition::decode_holder))
}

pub fn matches(predicate: Option<&Holder<LootItemCondition>>, context: &mut LootContext) -> bool {
    predicate.is_none_or(|predicate| predicate.value().test(context))
}

pub fn matches_entity(level: &mut ServerLevel, player: EntityId, predicate: Option<&Holder<LootItemCondition>>, entity: EntityId) -> bool {
    predicate.is_none_or(|predicate| predicate.value().test(&mut EntityPredicate::create_context(level, player, entity)))
}
