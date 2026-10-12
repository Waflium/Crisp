use super::abstract_cooking_recipe::AbstractCookingRecipe;
use super::brewing_recipe::BrewingRecipe;
use super::crafting_recipe::CraftingRecipe;
use super::recipe::Recipe;
use super::smithing_recipe::SmithingRecipe;
use super::stonecutter_recipe::StonecutterRecipe;

pub struct RecipeHolder {
    pub id: &'static str,
    pub value: Recipe,
}

impl RecipeHolder {
    pub fn crafting(&self) -> Option<&CraftingRecipe> {
        match &self.value {
            Recipe::Crafting(recipe) => Some(recipe),
            _ => None,
        }
    }

    pub fn cooking(&self) -> Option<&AbstractCookingRecipe> {
        match &self.value {
            Recipe::Cooking(recipe) => Some(recipe),
            _ => None,
        }
    }

    pub fn stonecutter(&self) -> Option<&StonecutterRecipe> {
        match &self.value {
            Recipe::Stonecutter(recipe) => Some(recipe),
            _ => None,
        }
    }

    pub fn smithing(&self) -> Option<&SmithingRecipe> {
        match &self.value {
            Recipe::Smithing(recipe) => Some(recipe),
            _ => None,
        }
    }

    pub fn brewing(&self) -> Option<&BrewingRecipe> {
        match &self.value {
            Recipe::Brewing(recipe) => Some(recipe),
            _ => None,
        }
    }
}
