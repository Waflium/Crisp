use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::level::block::entity::lectern_block_entity::DATA_PAGE;

pub const BUTTON_PREV_PAGE: i32 = 1;
pub const BUTTON_NEXT_PAGE: i32 = 2;
pub const BUTTON_TAKE_BOOK: i32 = 3;
pub const BUTTON_PAGE_JUMP_RANGE_START: i32 = 100;

impl ServerLevel {
    pub fn lectern_menu_click_menu_button(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, button_id: i32) -> bool {
        if button_id >= BUTTON_PAGE_JUMP_RANGE_START {
            let page_to_set = button_id - BUTTON_PAGE_JUMP_RANGE_START;
            self.lectern_menu_set_data(menu, player, DATA_PAGE, page_to_set);
            return true;
        }
        match button_id {
            BUTTON_PREV_PAGE => {
                let current_page = self.lectern_menu_get_page(menu);
                self.lectern_menu_set_data(menu, player, DATA_PAGE, current_page - 1);
                true
            }
            BUTTON_NEXT_PAGE => {
                let current_page = self.lectern_menu_get_page(menu);
                self.lectern_menu_set_data(menu, player, DATA_PAGE, current_page + 1);
                true
            }
            BUTTON_TAKE_BOOK => {
                if !self.may_build(player) {
                    return false;
                }
                let book = self.remove_item_no_update(menu.container, 0);
                self.container_set_changed(menu.container);
                if self.player_mut(player).pick_up(book.clone()) > 0 {
                    self.player_drop(player, book, false, Prediction::Predicted);
                }
                true
            }
            _ => false,
        }
    }

    fn lectern_menu_set_data(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, id: usize, value: i32) {
        if let ContainerRef::Lectern { pos, .. } = menu.container
            && id == DATA_PAGE
        {
            self.lectern_set_page(pos, value);
        }
        self.menu_broadcast_changes(menu, player);
    }

    fn lectern_menu_get_page(&mut self, menu: &AbstractContainerMenu) -> i32 {
        match menu.container {
            ContainerRef::Lectern { pos, .. } => self.lectern(pos).map_or(0, |lectern| lectern.get_page()),
            _ => 0,
        }
    }

    pub fn menu_click_menu_button(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, button_id: i32) -> bool {
        match menu.kind {
            MenuKind::Lectern => self.lectern_menu_click_menu_button(menu, player, button_id),
            MenuKind::Enchantment => self.enchantment_menu_click_menu_button(menu, player, button_id),
            _ => false,
        }
    }
}
