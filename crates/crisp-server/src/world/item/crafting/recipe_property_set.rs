use std::sync::OnceLock;

use crisp_data::Item;

use super::brewing_recipe::brewing_recipes;
use crate::world::item::ItemStack;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecipePropertySetKey {
    BrewingInputs,
    BrewingReagents,
}

pub const BREWING_INPUTS: RecipePropertySetKey = RecipePropertySetKey::BrewingInputs;
pub const BREWING_REAGENTS: RecipePropertySetKey = RecipePropertySetKey::BrewingReagents;

pub struct RecipePropertySet {
    items: Vec<Item>,
}

impl RecipePropertySet {
    fn create(items: impl Iterator<Item = Item>) -> RecipePropertySet {
        let mut collected: Vec<Item> = Vec::new();
        for item in items {
            if !collected.contains(&item) {
                collected.push(item);
            }
        }
        RecipePropertySet { items: collected }
    }

    pub fn get(id: RecipePropertySetKey) -> &'static RecipePropertySet {
        static BREWING_INPUTS_SET: OnceLock<RecipePropertySet> = OnceLock::new();
        static BREWING_REAGENTS_SET: OnceLock<RecipePropertySet> = OnceLock::new();
        match id {
            RecipePropertySetKey::BrewingInputs => {
                BREWING_INPUTS_SET.get_or_init(|| RecipePropertySet::create(brewing_recipes().iter().flat_map(|r| r.get_input().ingredient.iter().copied())))
            }
            RecipePropertySetKey::BrewingReagents => {
                BREWING_REAGENTS_SET.get_or_init(|| RecipePropertySet::create(brewing_recipes().iter().flat_map(|r| r.get_reagent().ingredient.iter().copied())))
            }
        }
    }

    pub fn test(&self, item_stack: &ItemStack) -> bool {
        self.items.contains(&item_stack.item)
    }
}
