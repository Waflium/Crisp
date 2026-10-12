use crate::world::entity::player::stacked_item_contents::StackedItemContents;
use crate::world::item::ItemStack;
use crate::world::item::crafting::crafting_input::{CraftingInput, Positioned};

#[derive(Clone, Debug)]
pub struct TransientCraftingContainer {
    pub items: Vec<ItemStack>,
    width: usize,
    height: usize,
}

impl TransientCraftingContainer {
    pub fn new(width: usize, height: usize) -> TransientCraftingContainer {
        TransientCraftingContainer { items: vec![ItemStack::empty(); width * height], width, height }
    }

    pub fn get_width(&self) -> usize {
        self.width
    }

    pub fn get_height(&self) -> usize {
        self.height
    }

    pub fn get_items(&self) -> &[ItemStack] {
        &self.items
    }

    pub fn as_craft_input(&self) -> CraftingInput {
        self.as_positioned_craft_input().input
    }

    pub fn as_positioned_craft_input(&self) -> Positioned {
        CraftingInput::of_positioned(self.width, self.height, &self.items)
    }

    pub fn fill_stacked_contents(&self, contents: &mut StackedItemContents) {
        for item_stack in &self.items {
            contents.account_simple_stack(item_stack);
        }
    }
}
