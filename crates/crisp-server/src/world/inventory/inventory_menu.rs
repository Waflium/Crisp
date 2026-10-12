use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};

pub const CONTAINER_ID: i32 = 0;
const INPUT_GRID_SLOTS: [usize; 4] = [1, 2, 3, 4];

impl AbstractContainerMenu {
    pub fn inventory() -> AbstractContainerMenu {
        AbstractContainerMenu::new(CONTAINER_ID, MenuKind::Inventory, None)
    }

    pub fn inventory_menu_get_input_grid_slots(&self) -> &'static [usize] {
        &INPUT_GRID_SLOTS
    }
}
