use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::crafting::recipe_holder::RecipeHolder;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PostPlaceAction {
    Nothing,
    PlaceGhostRecipe,
}

impl AbstractContainerMenu {
    pub fn is_recipe_book_menu(&self) -> bool {
        matches!(self.kind, MenuKind::Inventory | MenuKind::Crafting | MenuKind::Furnace(_))
    }
}

impl ServerLevel {
    pub fn recipe_book_menu_handle_placement(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, use_max_items: bool, allow_dropping_items_to_clear: bool, recipe: &RecipeHolder) -> Option<PostPlaceAction> {
        match menu.kind {
            MenuKind::Inventory | MenuKind::Crafting => self.abstract_crafting_menu_handle_placement(menu, player, use_max_items, allow_dropping_items_to_clear, recipe),
            MenuKind::Furnace(_) => self.abstract_furnace_menu_handle_placement(menu, player, use_max_items, allow_dropping_items_to_clear, recipe),
            _ => Some(PostPlaceAction::Nothing),
        }
    }
}
