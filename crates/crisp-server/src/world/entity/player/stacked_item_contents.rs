use crisp_data::Item;

use super::stacked_contents::StackedContents;
use crate::world::item::crafting::ingredient::Ingredient;
use crate::world::item::{Inventory, ItemStack};
use crate::world::item::crafting::recipe::Recipe;

#[derive(Default)]
pub struct StackedItemContents {
    raw: StackedContents<Item>,
}

impl StackedItemContents {
    pub fn account_simple_stack(&mut self, item_stack: &ItemStack) {
        if Inventory::is_usable_for_crafting(item_stack) {
            self.account_stack(item_stack);
        }
    }

    pub fn account_stack(&mut self, item_stack: &ItemStack) {
        self.account_stack_with_max(item_stack, item_stack.max_stack_size());
    }

    pub fn account_stack_with_max(&mut self, item_stack: &ItemStack, max_count: i32) {
        if !item_stack.is_empty() {
            let count = max_count.min(item_stack.count);
            self.raw.account(item_stack.item, count);
        }
    }

    pub fn can_craft(&mut self, recipe: &Recipe, output: Option<&mut dyn FnMut(Item)>) -> bool {
        self.can_craft_amount(recipe, 1, output)
    }

    pub fn can_craft_amount(&mut self, recipe: &Recipe, amount: i32, output: Option<&mut dyn FnMut(Item)>) -> bool {
        let placement_info = recipe.placement_info();
        if placement_info.is_impossible_to_place() { false } else { self.raw.try_pick(placement_info.ingredients(), amount, output) }
    }

    pub fn can_craft_ingredients(&mut self, contents: &[Ingredient], output: Option<&mut dyn FnMut(Item)>) -> bool {
        self.raw.try_pick(contents, 1, output)
    }

    pub fn get_biggest_craftable_stack(&mut self, recipe: &Recipe, max_size: i32, output: Option<&mut dyn FnMut(Item)>) -> i32 {
        self.raw.try_pick_all(recipe.placement_info().ingredients(), max_size, output)
    }

    pub fn clear(&mut self) {
        self.raw.clear();
    }
}
