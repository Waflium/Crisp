use super::abstract_container_menu::AbstractContainerMenu;
use super::recipe_book_menu::PostPlaceAction;
use crate::recipebook::server_place_recipe::{self, CraftingMenuAccess};
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::entity::player::stacked_item_contents::StackedItemContents;
use crate::world::item::ItemStack;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::single_recipe_input::SingleRecipeInput;

const INPUT_GRID_SLOTS: [usize; 1] = [0];
const SLOTS_TO_CLEAR: [usize; 2] = [0, 2];

struct AbstractFurnaceMenuAccess;

impl CraftingMenuAccess for AbstractFurnaceMenuAccess {
    fn fill_craft_slots_stacked_contents(&self, level: &mut ServerLevel, menu: &AbstractContainerMenu, stacked_contents: &mut StackedItemContents) {
        level.abstract_furnace_menu_fill_craft_slots_stacked_contents(menu, stacked_contents);
    }

    fn clear_crafting_content(&self, level: &mut ServerLevel, menu: &mut AbstractContainerMenu, player: EntityId) {
        for slot in SLOTS_TO_CLEAR {
            level.slot_set(menu, player, slot, ItemStack::empty());
        }
    }

    fn recipe_matches(&self, level: &mut ServerLevel, menu: &AbstractContainerMenu, recipe: &RecipeHolder) -> Option<bool> {
        let cooking_recipe = recipe.cooking()?;
        let item = menu.container.map_or_else(ItemStack::empty, |container| level.get_item(container, 0));
        Some(cooking_recipe.matches(&SingleRecipeInput { item: &item }))
    }
}

impl ServerLevel {
    pub fn abstract_furnace_menu_fill_craft_slots_stacked_contents(&mut self, menu: &AbstractContainerMenu, stacked_contents: &mut StackedItemContents) {
        if let Some(pos) = menu.container.and_then(ContainerRef::single_block)
            && let Some(furnace) = self.furnace(pos)
        {
            furnace.fill_stacked_contents(stacked_contents);
        }
    }

    pub fn abstract_furnace_menu_handle_placement(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, use_max_items: bool, allow_dropping_items_to_clear: bool, recipe: &RecipeHolder) -> Option<PostPlaceAction> {
        server_place_recipe::place_recipe(self, menu, &AbstractFurnaceMenuAccess, 1, 1, &INPUT_GRID_SLOTS, &SLOTS_TO_CLEAR, player, recipe, use_max_items, allow_dropping_items_to_clear)
    }
}
