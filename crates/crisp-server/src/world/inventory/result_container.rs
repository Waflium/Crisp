use crisp_data::game_rule;

use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;

#[derive(Clone, Debug)]
pub struct ResultContainer {
    pub item: ItemStack,
    recipe_used: Option<&'static str>,
}

impl Default for ResultContainer {
    fn default() -> ResultContainer {
        ResultContainer { item: ItemStack::empty(), recipe_used: None }
    }
}

impl ResultContainer {
    pub fn set_recipe_used_by(&mut self, level: &ServerLevel, recipe: &'static RecipeHolder) -> bool {
        if recipe.value.is_special() || !level.game_rules.get_bool(game_rule::LIMITED_CRAFTING) {
            self.recipe_used = Some(recipe.id);
            return true;
        }
        false
    }

    pub fn set_recipe_used(&mut self, recipe_used: Option<&'static str>) {
        self.recipe_used = recipe_used;
    }

    pub fn get_recipe_used(&self) -> Option<&'static str> {
        self.recipe_used
    }

    pub fn award_used_recipes(&mut self) {
        if let Some(recipe) = self.recipe_used.and_then(|id| RecipeManager::get().by_key(id))
            && !recipe.value.is_special()
        {
            self.set_recipe_used(None);
        }
    }
}
