use crisp_core::BlockPos;
use crisp_data::{item_tags, items, registry, registry_id};

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::map_post_processing::MapPostProcessing;
use crate::world::item::map_item;
use crate::world::level::saveddata::maps::map_item_saved_data::MAX_SCALE;

pub const MAP_SLOT: usize = 0;
pub const ADDITIONAL_SLOT: usize = 1;
pub const RESULT_SLOT: usize = 2;
const INV_SLOT_START: usize = 3;
const INV_SLOT_END: usize = 30;
const USE_ROW_SLOT_START: usize = 30;
const USE_ROW_SLOT_END: usize = 39;
pub const CONTAINER: usize = 0;
pub const RESULT_CONTAINER: usize = 1;

#[derive(Default)]
pub struct CartographyTableMenu {
    last_sound_time: i64,
}

pub fn map_slot_may_place(item_stack: &ItemStack) -> bool {
    item_stack.has(dc::MAP_ID)
}

pub fn additional_slot_may_place(item_stack: &ItemStack) -> bool {
    item_stack.is(items::PAPER) || item_stack.is(items::MAP) || item_stack.is(items::GLASS_PANE)
}

impl AbstractContainerMenu {
    pub fn cartography_table(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::CartographyTable, None);
        menu.access = Some(pos);
        menu.cartography_table = Some(CartographyTableMenu::default());
        menu
    }

    fn cartography_table_state(&mut self) -> &mut CartographyTableMenu {
        self.cartography_table.as_mut().expect("a cartography table menu")
    }
}

impl ServerLevel {
    pub fn cartography_table_menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let map_stack = menu.containers[CONTAINER].get_item(0).clone();
        let additional_stack = menu.containers[CONTAINER].get_item(1).clone();
        let result_stack = menu.containers[RESULT_CONTAINER].get_item(2).clone();
        if result_stack.is_empty() || !map_stack.is_empty() && !additional_stack.is_empty() {
            if !map_stack.is_empty() && !additional_stack.is_empty() {
                self.cartography_table_menu_setup_result_slot(menu, player, &map_stack, &additional_stack, &result_stack);
            }
        } else {
            self.menu_container_remove_item_no_update(menu, RESULT_CONTAINER, 2);
        }
    }

    fn cartography_table_menu_setup_result_slot(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, map_stack: &ItemStack, additional_stack: &ItemStack, result_stack: &ItemStack) {
        if menu.access.is_none() {
            return;
        }
        let Some(map_data) = map_item::get_saved_data(map_stack, self) else { return };
        let (locked, scale) = {
            let map_data = map_data.borrow();
            (map_data.locked, map_data.scale)
        };
        let result = if additional_stack.is(items::PAPER) && map_stack.get_item().is(&item_tags::EXTENDABLE_MAPS) && !locked && scale < MAX_SCALE {
            let mut result = map_stack.copy_with_count(1);
            result.set(dc::MAP_POST_PROCESSING, Some(MapPostProcessing::Scale.to_nbt()));
            self.menu_broadcast_changes(menu, player);
            result
        } else if additional_stack.is(items::GLASS_PANE) && !locked {
            let mut result = map_stack.copy_with_count(1);
            result.set(dc::MAP_POST_PROCESSING, Some(MapPostProcessing::Lock.to_nbt()));
            self.menu_broadcast_changes(menu, player);
            result
        } else {
            if !additional_stack.is(items::MAP) {
                self.menu_container_remove_item_no_update(menu, RESULT_CONTAINER, 2);
                self.menu_broadcast_changes(menu, player);
                return;
            }
            let result = map_stack.copy_with_count(2);
            self.menu_broadcast_changes(menu, player);
            result
        };
        if !ItemStack::matches(&result, result_stack) {
            self.menu_container_set_item(menu, player, RESULT_CONTAINER, 2, result);
            self.menu_broadcast_changes(menu, player);
        }
    }

    pub fn cartography_table_result_slot_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, carried: &mut ItemStack) {
        self.slot_remove(menu, player, MAP_SLOT, 1);
        self.slot_remove(menu, player, ADDITIONAL_SLOT, 1);
        crate::world::item::on_crafted_by(carried, self, player);
        if let Some(pos) = menu.access {
            let game_time = self.game_time;
            let state = menu.cartography_table_state();
            if state.last_sound_time != game_time {
                self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:ui.cartography_table.take_result"), SoundSource::Blocks, 1.0, 1.0);
                menu.cartography_table_state().last_sound_time = game_time;
            }
        }
    }

    pub fn cartography_table_menu_quick_move_stack(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, stack: &mut ItemStack) -> bool {
        if index == RESULT_SLOT {
            crate::world::item::on_crafted_by(stack, self, player);
            self.move_item_stack_to(menu, player, stack, INV_SLOT_START, USE_ROW_SLOT_END, true)
        } else if index != ADDITIONAL_SLOT && index != MAP_SLOT {
            if stack.has(dc::MAP_ID) {
                self.move_item_stack_to(menu, player, stack, MAP_SLOT, MAP_SLOT + 1, false)
            } else if !additional_slot_may_place(stack) {
                if (INV_SLOT_START..INV_SLOT_END).contains(&index) {
                    self.move_item_stack_to(menu, player, stack, USE_ROW_SLOT_START, USE_ROW_SLOT_END, false)
                } else {
                    !((USE_ROW_SLOT_START..USE_ROW_SLOT_END).contains(&index) && !self.move_item_stack_to(menu, player, stack, INV_SLOT_START, INV_SLOT_END, false))
                }
            } else {
                self.move_item_stack_to(menu, player, stack, ADDITIONAL_SLOT, ADDITIONAL_SLOT + 1, false)
            }
        } else {
            self.move_item_stack_to(menu, player, stack, INV_SLOT_START, USE_ROW_SLOT_END, false)
        }
    }

    pub fn cartography_table_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        self.menu_container_remove_item_no_update(menu, RESULT_CONTAINER, 2);
        if menu.access.is_some() {
            self.clear_container(menu, player, CONTAINER);
        }
    }
}
