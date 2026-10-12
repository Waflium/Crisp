use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::crafting::recipe::Recipe;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct RecipeUnlockedTrigger(pub CriterionTrigger);

impl RecipeUnlockedTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, recipe: &str) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::RecipeUnlocked(t) = instance else { return false };
            t.matches(recipe)
        });
    }
}

pub fn decode_recipe_list(tag: &Nbt) -> DataResult<Vec<Option<String>>> {
    if let Nbt::String(name) = tag
        && let Some(location) = name.strip_prefix('#')
    {
        return Err(format!("Missing tag: '{}' in '{}'", crate::resources::identifier::read(location)?, reloadable_server_registries::RECIPE));
    }
    codec::compact_list_of(tag, |element| {
        let Ok(key) = codec::identifier(element) else {
            Recipe::decode(element)?;
            return Ok(None);
        };
        if reloadable_server_registries::contains_key(reloadable_server_registries::RECIPE, &key) {
            Ok(Some(key))
        } else {
            Err(format!("Missing element ResourceKey[{} / {key}]", reloadable_server_registries::RECIPE))
        }
    })
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    recipes: Vec<Option<String>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance { player: simple_criterion_trigger::decode_player(tag)?, recipes: codec::decode_field(tag, "recipes", decode_recipe_list)? })
    }

    pub fn matches(&self, recipe: &str) -> bool {
        self.recipes.iter().any(|holder| holder.as_deref() == Some(recipe))
    }
}
