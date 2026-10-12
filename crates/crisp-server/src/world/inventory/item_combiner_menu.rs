use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

pub const INPUT_SLOTS: usize = 0;
pub const RESULT_SLOTS: usize = 1;
const INVENTORY_SLOTS_PER_ROW: usize = 9;
const INVENTORY_ROWS: usize = 3;

impl AbstractContainerMenu {
    pub fn get_result_slot(&self) -> usize {
        match self.kind {
            MenuKind::Smithing => super::smithing_menu::RESULT_SLOT,
            _ => super::anvil_menu::RESULT_SLOT,
        }
    }

    fn get_inventory_slot_start(&self) -> usize {
        self.get_result_slot() + 1
    }

    fn get_inventory_slot_end(&self) -> usize {
        self.get_inventory_slot_start() + INVENTORY_SLOTS_PER_ROW * INVENTORY_ROWS
    }

    fn get_use_row_start(&self) -> usize {
        self.get_inventory_slot_end()
    }

    fn get_use_row_end(&self) -> usize {
        self.get_use_row_start() + INVENTORY_SLOTS_PER_ROW
    }
}

impl ServerLevel {
    pub fn item_combiner_menu_may_pickup(&mut self, menu: &AbstractContainerMenu, player: EntityId, has_item: bool) -> bool {
        match menu.kind {
            MenuKind::Anvil => self.anvil_menu_may_pickup(menu, player, has_item),
            _ => true,
        }
    }

    pub fn item_combiner_menu_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, carried: &ItemStack) {
        match menu.kind {
            MenuKind::Smithing => self.smithing_menu_on_take(menu, player, carried),
            _ => self.anvil_menu_on_take(menu, player, carried),
        }
    }

    fn item_combiner_menu_create_result(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        match menu.kind {
            MenuKind::Smithing => self.smithing_menu_create_result(menu, player),
            _ => self.anvil_menu_create_result(menu, player),
        }
    }

    pub fn item_combiner_menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, container: usize) {
        self.menu_broadcast_changes(menu, player);
        if container == INPUT_SLOTS {
            self.item_combiner_menu_create_result(menu, player);
        }
    }

    pub fn item_combiner_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        if menu.access.is_some() {
            self.clear_container(menu, player, INPUT_SLOTS);
        }
    }

    fn item_combiner_menu_can_move_into_input_slots(&mut self, menu: &AbstractContainerMenu, player: EntityId, stack: &ItemStack) -> bool {
        match menu.kind {
            MenuKind::Smithing => self.smithing_menu_can_move_into_input_slots(menu, player, stack),
            _ => true,
        }
    }

    pub fn item_combiner_menu_quick_move_stack(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, stack: &mut ItemStack) -> bool {
        let inventory_slot_start = menu.get_inventory_slot_start();
        let use_row_slot_end = menu.get_use_row_end();
        if index == menu.get_result_slot() {
            self.move_item_stack_to(menu, player, stack, inventory_slot_start, use_row_slot_end, true)
        } else if index < menu.get_result_slot() {
            self.move_item_stack_to(menu, player, stack, inventory_slot_start, use_row_slot_end, false)
        } else if self.item_combiner_menu_can_move_into_input_slots(menu, player, stack) && index >= menu.get_inventory_slot_start() && index < menu.get_use_row_end() {
            let result_slot = menu.get_result_slot();
            self.move_item_stack_to(menu, player, stack, 0, result_slot, false)
        } else if index >= menu.get_inventory_slot_start() && index < menu.get_inventory_slot_end() {
            let (use_row_start, use_row_end) = (menu.get_use_row_start(), menu.get_use_row_end());
            self.move_item_stack_to(menu, player, stack, use_row_start, use_row_end, false)
        } else {
            let (inventory_slot_start, inventory_slot_end) = (menu.get_inventory_slot_start(), menu.get_inventory_slot_end());
            !(index >= menu.get_use_row_start() && index < menu.get_use_row_end() && !self.move_item_stack_to(menu, player, stack, inventory_slot_start, inventory_slot_end, false))
        }
    }
}
