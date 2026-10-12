use std::sync::LazyLock;

use crisp_core::hash_map::FastMap;
use crisp_protocol::Writer;

use super::display::slot_display::write_holder_set;
use super::stonecutter_recipe::StonecutterRecipe;

use super::brewing_input::BrewingInput;
use super::crafting_input::CraftingInput;
use super::display::recipe_display_entry::{RecipeDisplayEntry, RecipeDisplayId};
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

pub struct ServerDisplayInfo {
    pub display: RecipeDisplayEntry,
    pub parent: &'static str,
}

pub struct RecipeManager {
    recipes: RecipeMap,
    property_sets: Vec<(RecipePropertySetKey, RecipePropertySet)>,
    stonecutter_recipes: Vec<&'static str>,
    all_displays: Vec<ServerDisplayInfo>,
    recipe_to_display: FastMap<&'static str, Vec<usize>>,
    learnable_recipes: Vec<&'static str>,
}

fn unpack_recipe_info(recipes: &[RecipeHolder]) -> Vec<ServerDisplayInfo> {
    let mut result = Vec::new();
    let mut recipe_groups: Vec<&str> = Vec::new();
    for recipe_holder in recipes {
        let recipe = &recipe_holder.value;
        let group_id = if recipe.group().is_empty() {
            None
        } else {
            Some(match recipe_groups.iter().position(|&group| group == recipe.group()) {
                Some(index) => index as i32,
                None => {
                    recipe_groups.push(recipe.group());
                    recipe_groups.len() as i32 - 1
                }
            })
        };
        let placement_check = (!recipe.is_special()).then(|| recipe.placement_info().ingredients().to_vec());
        for recipe_display in recipe.display() {
            let id = RecipeDisplayId(result.len() as i32);
            let display = RecipeDisplayEntry { id, display: recipe_display, group: group_id, category: recipe.recipe_book_category(), crafting_requirements: placement_check.clone() };
            result.push(ServerDisplayInfo { display, parent: recipe_holder.id });
        }
    }
    result
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

pub fn write_update_recipes(w: &mut Writer) {
    let recipe_manager = RecipeManager::get();
    w.varint(recipe_manager.property_sets().len() as i32);
    for (key, property_set) in recipe_manager.property_sets() {
        w.string(key.name());
        let items: Vec<_> = property_set.items().collect();
        w.varint(items.len() as i32);
        items.iter().for_each(|item| w.varint(item.0 as i32));
    }
    let stonecutter_recipes: Vec<&StonecutterRecipe> = recipe_manager.stonecutter_recipes().filter_map(RecipeHolder::stonecutter).collect();
    w.varint(stonecutter_recipes.len() as i32);
    for recipe in stonecutter_recipes {
        write_holder_set(w, recipe.input().values());
        recipe.result_display().write(w);
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
        let recipes = RecipeMap::create(holders);
        let learnable_recipes = recipes.values().iter().filter(|recipe| !recipe.value.is_special()).map(|recipe| recipe.id).collect();
        let mut manager = RecipeManager {
            recipes,
            property_sets: Vec::new(),
            stonecutter_recipes: Vec::new(),
            all_displays: Vec::new(),
            recipe_to_display: FastMap::default(),
            learnable_recipes,
        };
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
        self.all_displays = unpack_recipe_info(self.recipes.values());
        for (index, display_info) in self.all_displays.iter().enumerate() {
            self.recipe_to_display.entry(display_info.parent).or_default().push(index);
        }
    }

    pub fn get_learnable_recipes(&self) -> impl Iterator<Item = &RecipeHolder> {
        self.learnable_recipes.iter().filter_map(|id| self.recipes.by_key(id))
    }

    pub fn get_recipe_from_display(&self, id: RecipeDisplayId) -> Option<&ServerDisplayInfo> {
        usize::try_from(id.0).ok().and_then(|index| self.all_displays.get(index))
    }

    pub fn list_displays_for_recipe(&self, id: &str) -> impl Iterator<Item = &RecipeDisplayEntry> {
        self.recipe_to_display.get(id).into_iter().flatten().map(|&index| &self.all_displays[index].display)
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
