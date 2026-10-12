use crisp_core::BlockPos;
use crisp_data::{registry, registry_id};

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::crafting::selectable_recipe::SingleInputSet;

pub const INPUT_SLOT: usize = 0;
pub const RESULT_SLOT: usize = 1;
pub const INV_SLOT_START: usize = 2;
pub const INV_SLOT_END: usize = 29;
pub const USE_ROW_SLOT_START: usize = 29;
pub const USE_ROW_SLOT_END: usize = 38;
pub const CONTAINER: usize = 0;
pub const RESULT_CONTAINER: usize = 1;

pub struct StonecutterMenu {
    selected_recipe_index: i32,
    recipes_for_input: SingleInputSet,
    input: ItemStack,
    last_sound_time: i64,
}

impl StonecutterMenu {
    fn new() -> StonecutterMenu {
        StonecutterMenu { selected_recipe_index: 0, recipes_for_input: SingleInputSet::empty(), input: ItemStack::empty(), last_sound_time: 0 }
    }

    pub fn get_selected_recipe_index(&self) -> i32 {
        self.selected_recipe_index
    }

    fn is_valid_recipe_index(&self, button_id: i32) -> bool {
        button_id >= 0 && (button_id as usize) < self.recipes_for_input.size()
    }
}

impl AbstractContainerMenu {
    pub fn stonecutter(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Stonecutter, None);
        menu.access = Some(pos);
        menu.stonecutter = Some(StonecutterMenu::new());
        menu
    }

    fn stonecutter_state(&mut self) -> &mut StonecutterMenu {
        self.stonecutter.as_mut().expect("a stonecutter menu")
    }
}

impl ServerLevel {
    pub fn stonecutter_menu_click_menu_button(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, button_id: i32) -> bool {
        let state = menu.stonecutter_state();
        if state.selected_recipe_index == button_id {
            return false;
        }
        if state.is_valid_recipe_index(button_id) {
            state.selected_recipe_index = button_id;
            self.stonecutter_menu_setup_result_slot(menu, player, button_id);
        }
        true
    }

    pub fn stonecutter_menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let input = self.slot_get(menu, player, INPUT_SLOT);
        let state = menu.stonecutter_state();
        if !input.is(state.input.get_item()) {
            state.input = input.copy();
            self.stonecutter_menu_setup_recipe_list(menu, player, &input);
        }
    }

    fn stonecutter_menu_setup_recipe_list(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, item: &ItemStack) {
        menu.stonecutter_state().selected_recipe_index = -1;
        self.slot_set(menu, player, RESULT_SLOT, ItemStack::empty());
        menu.stonecutter_state().recipes_for_input = if !item.is_empty() { RecipeManager::get().stonecutter_recipes().select_by_input(item) } else { SingleInputSet::empty() };
    }

    fn stonecutter_menu_setup_result_slot(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: i32) {
        let state = menu.stonecutter_state();
        let used_recipe = if !state.recipes_for_input.is_empty() && state.is_valid_recipe_index(index) { Some(state.recipes_for_input.entries()[index as usize].recipe) } else { None };
        match used_recipe {
            Some(recipe) => {
                menu.containers[RESULT_CONTAINER].result().set_recipe_used(Some(recipe.id));
                let result = recipe.stonecutter().expect("a stonecutter recipe").assemble();
                self.slot_set(menu, player, RESULT_SLOT, result);
            }
            None => {
                self.slot_set(menu, player, RESULT_SLOT, ItemStack::empty());
                menu.containers[RESULT_CONTAINER].result().set_recipe_used(None);
            }
        }
        self.menu_broadcast_changes(menu, player);
    }

    pub fn stonecutter_result_slot_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, carried: &mut ItemStack) {
        let count = carried.count;
        carried.on_crafted_by(self, player, count);
        let relevant_items = [self.slot_get(menu, player, INPUT_SLOT)];
        menu.containers[RESULT_CONTAINER].award_used_recipes(self, player, &relevant_items);
        let remaining = self.slot_remove(menu, player, INPUT_SLOT, 1);
        if !remaining.is_empty() {
            let selected_recipe_index = menu.stonecutter_state().selected_recipe_index;
            self.stonecutter_menu_setup_result_slot(menu, player, selected_recipe_index);
        }
        if let Some(pos) = menu.access {
            let game_time = self.game_time;
            let state = menu.stonecutter_state();
            if state.last_sound_time != game_time {
                state.last_sound_time = game_time;
                self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:ui.stonecutter.take_result"), SoundSource::Blocks, 1.0, 1.0);
            }
        }
    }

    pub fn stonecutter_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        self.menu_container_remove_item_no_update(menu, RESULT_CONTAINER, 1);
        if menu.access.is_some() {
            self.clear_container(menu, player, CONTAINER);
        }
    }
}
