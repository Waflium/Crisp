use std::sync::LazyLock;

use super::brewing_input::BrewingInput;
use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use super::recipe::Recipe;
use super::recipe_holder::RecipeHolder;
use super::recipe_map::RecipeMap;
use super::recipe_property_set::{RecipePropertySet, RecipePropertySetKey};
use super::recipe_type::RecipeType;
use super::single_recipe_input::SingleRecipeInput;
use super::smithing_recipe_input::SmithingRecipeInput;
use crate::server::level::ServerLevel;
use crate::world::item::component::json_to_nbt;

pub struct RecipeManager {
    recipes: RecipeMap,
    property_sets: Vec<(RecipePropertySetKey, RecipePropertySet)>,
    stonecutter_recipes: Vec<&'static str>,
}

static RECIPE_MANAGER: LazyLock<RecipeManager> = LazyLock::new(RecipeManager::new);

fn extract_ingredient(key: RecipePropertySetKey, recipe: &Recipe) -> Option<&Ingredient> {
    let for_single_input = |ty: RecipeType| match recipe {
        Recipe::Cooking(cooking) if cooking.get_type() == ty => Some(cooking.input()),
        _ => None,
    };
    match key {
        RecipePropertySetKey::SmithingAddition => match recipe {
            Recipe::Smithing(smithing) => smithing.addition_ingredient(),
            _ => None,
        },
        RecipePropertySetKey::SmithingBase => match recipe {
            Recipe::Smithing(smithing) => Some(smithing.base_ingredient()),
            _ => None,
        },
        RecipePropertySetKey::SmithingTemplate => match recipe {
            Recipe::Smithing(smithing) => smithing.template_ingredient(),
            _ => None,
        },
        RecipePropertySetKey::FurnaceInput => for_single_input(RecipeType::Smelting),
        RecipePropertySetKey::BlastFurnaceInput => for_single_input(RecipeType::Blasting),
        RecipePropertySetKey::SmokerInput => for_single_input(RecipeType::Smoking),
        RecipePropertySetKey::CampfireInput => for_single_input(RecipeType::CampfireCooking),
        RecipePropertySetKey::BrewingInputs => match recipe {
            Recipe::Brewing(brewing) => Some(&brewing.get_input().ingredient),
            _ => None,
        },
        RecipePropertySetKey::BrewingReagents => match recipe {
            Recipe::Brewing(brewing) => Some(&brewing.get_reagent().ingredient),
            _ => None,
        },
    }
}

impl RecipeManager {
    pub fn get() -> &'static RecipeManager {
        &RECIPE_MANAGER
    }

    fn new() -> RecipeManager {
        let mut holders = Vec::new();
        for &(id, json) in crisp_data::RECIPES {
            let value: serde_json::Value = match serde_json::from_str(json) {
                Ok(value) => value,
                Err(error) => {
                    error!("Couldn't parse data file '{id}': {error}");
                    continue;
                }
            };
            match Recipe::decode(&json_to_nbt(&value)) {
                Ok(recipe) => holders.push(RecipeHolder { id, value: recipe }),
                Err(error) => error!("Couldn't parse data file '{id}': {error}"),
            }
        }
        let mut manager = RecipeManager { recipes: RecipeMap::create(holders), property_sets: Vec::new(), stonecutter_recipes: Vec::new() };
        manager.finalize_recipe_loading();
        manager
    }

    fn finalize_recipe_loading(&mut self) {
        let mut collectors: Vec<(RecipePropertySetKey, Vec<Ingredient>)> = RecipePropertySetKey::VALUES.iter().map(|&key| (key, Vec::new())).collect();
        for holder in self.recipes.values() {
            let recipe = &holder.value;
            if !recipe.is_special() && recipe.placement_info().is_impossible_to_place() {
                warn!("Recipe {} can't be placed due to empty ingredients and will be ignored", holder.id);
                continue;
            }
            for (key, ingredients) in &mut collectors {
                if let Some(ingredient) = extract_ingredient(*key, recipe) {
                    ingredients.push(ingredient.clone());
                }
            }
            if let Recipe::Stonecutter(_) = recipe {
                self.stonecutter_recipes.push(holder.id);
            }
        }
        self.property_sets = collectors.into_iter().map(|(key, ingredients)| (key, RecipePropertySet::create(&ingredients))).collect();
    }

    pub fn property_set(&self, id: RecipePropertySetKey) -> &RecipePropertySet {
        &self.property_sets.iter().find(|(key, _)| *key == id).expect("every property set is collected").1
    }

    pub fn property_sets(&self) -> &[(RecipePropertySetKey, RecipePropertySet)] {
        &self.property_sets
    }

    pub fn stonecutter_recipes(&self) -> impl Iterator<Item = &RecipeHolder> {
        self.stonecutter_recipes.iter().filter_map(|id| self.recipes.by_key(id))
    }

    pub fn get_recipes(&self) -> &[RecipeHolder] {
        self.recipes.values()
    }

    pub fn by_key(&self, recipe_id: &str) -> Option<&RecipeHolder> {
        self.recipes.by_key(recipe_id)
    }

    fn by_key_typed(&self, ty: RecipeType, recipe_id: &str) -> Option<&RecipeHolder> {
        self.recipes.by_key(recipe_id).filter(|recipe| recipe.value.get_type() == ty)
    }

    pub fn get_crafting_recipe_for(&self, input: &mut CraftingInput, level: &mut ServerLevel, recipe_hint: Option<&str>) -> Option<&RecipeHolder> {
        if let Some(hinted) = recipe_hint.and_then(|hint| self.by_key_typed(RecipeType::Crafting, hint))
            && hinted.crafting().is_some_and(|recipe| recipe.matches(input, level))
        {
            return Some(hinted);
        }
        if input.is_empty() {
            return None;
        }
        self.recipes.by_type(RecipeType::Crafting).find(|holder| holder.crafting().is_some_and(|recipe| recipe.matches(input, level)))
    }

    pub fn get_cooking_recipe_for(&self, ty: RecipeType, input: &SingleRecipeInput, recipe_hint: Option<&str>) -> Option<&RecipeHolder> {
        if let Some(hinted) = recipe_hint.and_then(|hint| self.by_key_typed(ty, hint))
            && hinted.cooking().is_some_and(|recipe| recipe.matches(input))
        {
            return Some(hinted);
        }
        if input.is_empty() {
            return None;
        }
        self.recipes.by_type(ty).find(|holder| holder.cooking().is_some_and(|recipe| recipe.matches(input)))
    }

    pub fn get_smithing_recipe_for(&self, input: &SmithingRecipeInput, recipe_hint: Option<&str>) -> Option<&RecipeHolder> {
        if let Some(hinted) = recipe_hint.and_then(|hint| self.by_key_typed(RecipeType::Smithing, hint))
            && hinted.smithing().is_some_and(|recipe| recipe.matches(input))
        {
            return Some(hinted);
        }
        if input.is_empty() {
            return None;
        }
        self.recipes.by_type(RecipeType::Smithing).find(|holder| holder.smithing().is_some_and(|recipe| recipe.matches(input)))
    }

    pub fn get_brewing_recipe_for(&self, input: &BrewingInput, recipe_hint: Option<&str>) -> Option<&RecipeHolder> {
        if let Some(hinted) = recipe_hint.and_then(|hint| self.by_key_typed(RecipeType::Brewing, hint))
            && hinted.brewing().is_some_and(|recipe| recipe.matches(input))
        {
            return Some(hinted);
        }
        if input.is_empty() {
            return None;
        }
        self.recipes.by_type(RecipeType::Brewing).find(|holder| holder.brewing().is_some_and(|recipe| recipe.matches(input)))
    }
}
