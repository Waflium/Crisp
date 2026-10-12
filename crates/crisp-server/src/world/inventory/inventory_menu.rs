use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};

pub const CONTAINER_ID: i32 = 0;

impl AbstractContainerMenu {
    pub fn inventory() -> AbstractContainerMenu {
        AbstractContainerMenu::new(CONTAINER_ID, MenuKind::Inventory, None)
    }
}
