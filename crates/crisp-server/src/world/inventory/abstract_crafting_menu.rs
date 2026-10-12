use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::crafting_menu::{CRAFT_SLOTS, RESULT_SLOTS};
use super::recipe_book_menu::PostPlaceAction;
use crate::recipebook::server_place_recipe::{self, CraftingMenuAccess};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::player::stacked_item_contents::StackedItemContents;
use crate::world::item::crafting::recipe_holder::RecipeHolder;

struct AbstractCraftingMenuAccess;

impl CraftingMenuAccess for AbstractCraftingMenuAccess {
    fn fill_craft_slots_stacked_contents(&self, _level: &mut ServerLevel, menu: &AbstractContainerMenu, stacked_contents: &mut StackedItemContents) {
        menu.abstract_crafting_menu_fill_craft_slots_stacked_contents(stacked_contents);
    }

    fn clear_crafting_content(&self, _level: &mut ServerLevel, menu: &mut AbstractContainerMenu, _player: EntityId) {
        menu.containers[RESULT_SLOTS].clear_content();
        menu.containers[CRAFT_SLOTS].clear_content();
    }

    fn recipe_matches(&self, level: &mut ServerLevel, menu: &AbstractContainerMenu, recipe: &RecipeHolder) -> Option<bool> {
        Some(recipe.crafting()?.matches(&mut menu.containers[CRAFT_SLOTS].crafting().as_craft_input(), level))
    }
}

impl AbstractContainerMenu {
    fn begin_placing_recipe(&mut self) {
        if self.kind == MenuKind::Crafting {
            self.crafting_menu_begin_placing_recipe();
        }
    }

    fn get_input_grid_slots(&self) -> &'static [usize] {
        match self.kind {
            MenuKind::Inventory => self.inventory_menu_get_input_grid_slots(),
            _ => self.crafting_menu_get_input_grid_slots(),
        }
    }

    pub fn get_grid_width(&self) -> i32 {
        self.containers[CRAFT_SLOTS].crafting().get_width() as i32
    }

    pub fn get_grid_height(&self) -> i32 {
        self.containers[CRAFT_SLOTS].crafting().get_height() as i32
    }

    pub fn abstract_crafting_menu_fill_craft_slots_stacked_contents(&self, stacked_contents: &mut StackedItemContents) {
        self.containers[CRAFT_SLOTS].crafting().fill_stacked_contents(stacked_contents);
    }
}

impl ServerLevel {
    pub fn abstract_crafting_menu_handle_placement(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, use_max_items: bool, allow_dropping_items_to_clear: bool, recipe: &RecipeHolder) -> Option<PostPlaceAction> {
        menu.begin_placing_recipe();
        let input_slots = menu.get_input_grid_slots();
        let (width, height) = (menu.get_grid_width(), menu.get_grid_height());
        let post_place_action =
            server_place_recipe::place_recipe(self, menu, &AbstractCraftingMenuAccess, width, height, input_slots, input_slots, player, recipe, use_max_items, allow_dropping_items_to_clear);
        self.finish_placing_recipe(menu, player, recipe)?;
        post_place_action
    }

    fn finish_placing_recipe(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, recipe: &RecipeHolder) -> Option<()> {
        if menu.kind == MenuKind::Crafting {
            return self.crafting_menu_finish_placing_recipe(menu, player, recipe);
        }
        Some(())
    }
}
