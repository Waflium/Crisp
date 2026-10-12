use crisp_core::BlockPos;
use crisp_data::{item_tags, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::core::holder_set::HolderSet;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::banner_item::is_banner_item;
use crate::world::item::component as dc;
use crate::world::item::component::codec;

pub const BANNER_SLOT: usize = 0;
pub const DYE_SLOT: usize = 1;
pub const PATTERN_SLOT: usize = 2;
pub const RESULT_SLOT: usize = 3;
const PATTERN_NOT_SET: i32 = -1;
const INV_SLOT_START: usize = 4;
const INV_SLOT_END: usize = 31;
const USE_ROW_SLOT_START: usize = 31;
const USE_ROW_SLOT_END: usize = 40;
pub const INPUT_CONTAINER: usize = 0;
pub const OUTPUT_CONTAINER: usize = 1;

#[derive(Default)]
pub struct LoomMenu {
    selected_banner_pattern_index: i32,
    selectable_patterns: Vec<i32>,
    last_sound_time: i64,
}

impl LoomMenu {
    pub fn get_selected_banner_pattern_index(&self) -> i32 {
        self.selected_banner_pattern_index
    }

    fn is_valid_pattern_index(&self, selected_pattern: i32) -> bool {
        selected_pattern >= 0 && (selected_pattern as usize) < self.selectable_patterns.len()
    }
}

pub fn is_pattern_item(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::LOOM_PATTERNS) && item_stack.has(dc::PROVIDES_BANNER_PATTERNS)
}

pub fn is_dye_item(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::LOOM_DYES) && item_stack.has(dc::DYE)
}

fn get_selectable_patterns(pattern_stack: &ItemStack) -> Vec<i32> {
    if pattern_stack.is_empty() {
        return crisp_data::tag("minecraft:banner_pattern", "minecraft:no_item_required").to_vec();
    }
    pattern_stack
        .get(dc::PROVIDES_BANNER_PATTERNS)
        .and_then(|item_patterns| HolderSet::decode("minecraft:banner_pattern", item_patterns).ok())
        .map_or_else(Vec::new, |item_patterns| item_patterns.contents().to_vec())
}

impl AbstractContainerMenu {
    pub fn loom(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Loom, None);
        menu.access = Some(pos);
        menu.loom = Some(LoomMenu::default());
        menu
    }

    fn loom_state(&mut self) -> &mut LoomMenu {
        self.loom.as_mut().expect("a loom menu")
    }
}

impl ServerLevel {
    pub fn loom_menu_click_menu_button(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, button_id: i32) -> bool {
        let state = menu.loom_state();
        if button_id >= 0 && (button_id as usize) < state.selectable_patterns.len() {
            state.selected_banner_pattern_index = button_id;
            let pattern = state.selectable_patterns[button_id as usize];
            self.loom_menu_setup_result_slot(menu, player, pattern);
            true
        } else {
            false
        }
    }

    pub fn loom_menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let banner_stack = self.slot_get(menu, player, BANNER_SLOT);
        let dye_stack = self.slot_get(menu, player, DYE_SLOT);
        let pattern_stack = self.slot_get(menu, player, PATTERN_SLOT);
        if banner_stack.is_empty() || dye_stack.is_empty() {
            self.slot_set(menu, player, RESULT_SLOT, ItemStack::empty());
            let state = menu.loom_state();
            state.selectable_patterns = Vec::new();
            state.selected_banner_pattern_index = PATTERN_NOT_SET;
            return;
        }
        let state = menu.loom_state();
        let selected_pattern = state.selected_banner_pattern_index;
        let valid_pattern_index = state.is_valid_pattern_index(selected_pattern);
        let previous_selectable_patterns = std::mem::replace(&mut state.selectable_patterns, get_selectable_patterns(&pattern_stack));
        let pattern_to_display = if state.selectable_patterns.len() == 1 {
            state.selected_banner_pattern_index = 0;
            Some(state.selectable_patterns[0])
        } else if !valid_pattern_index {
            state.selected_banner_pattern_index = PATTERN_NOT_SET;
            None
        } else {
            let selected_value = previous_selectable_patterns[selected_pattern as usize];
            match state.selectable_patterns.iter().position(|&pattern| pattern == selected_value) {
                Some(new_selected_index) => {
                    state.selected_banner_pattern_index = new_selected_index as i32;
                    Some(selected_value)
                }
                None => {
                    state.selected_banner_pattern_index = PATTERN_NOT_SET;
                    None
                }
            }
        };
        match pattern_to_display {
            Some(pattern) => {
                let has_max_patterns = banner_stack.get(dc::BANNER_PATTERNS).map_or(0, |patterns| patterns.as_list().len()) >= 6;
                if has_max_patterns {
                    menu.loom_state().selected_banner_pattern_index = PATTERN_NOT_SET;
                    self.slot_set(menu, player, RESULT_SLOT, ItemStack::empty());
                } else {
                    self.loom_menu_setup_result_slot(menu, player, pattern);
                }
            }
            None => self.slot_set(menu, player, RESULT_SLOT, ItemStack::empty()),
        }
        self.menu_broadcast_changes(menu, player);
    }

    fn loom_menu_setup_result_slot(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, pattern: i32) {
        let banner_stack = self.slot_get(menu, player, BANNER_SLOT);
        let dye_stack = self.slot_get(menu, player, DYE_SLOT);
        let mut result = ItemStack::empty();
        if !banner_stack.is_empty()
            && !dye_stack.is_empty()
            && let Some(pattern_color) = dye_stack.get(dc::DYE).cloned()
        {
            result = banner_stack.copy_with_count(1);
            let mut layers = result.get(dc::BANNER_PATTERNS).map_or_else(Vec::new, |layers| layers.as_list().to_vec());
            let pattern_name = codec::registry("minecraft:banner_pattern")[pattern as usize];
            layers.push(Nbt::Compound(vec![("pattern".into(), Nbt::String(pattern_name.into())), ("color".into(), pattern_color)]));
            result.set_value(dc::BANNER_PATTERNS, Nbt::List(layers));
        }
        if !ItemStack::matches(&result, &self.slot_get(menu, player, RESULT_SLOT)) {
            self.slot_set(menu, player, RESULT_SLOT, result);
        }
    }

    pub fn loom_result_slot_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        self.slot_remove(menu, player, BANNER_SLOT, 1);
        self.slot_remove(menu, player, DYE_SLOT, 1);
        if !self.slot_has_item(menu, player, BANNER_SLOT) || !self.slot_has_item(menu, player, DYE_SLOT) {
            menu.loom_state().selected_banner_pattern_index = PATTERN_NOT_SET;
        }
        if let Some(pos) = menu.access {
            let game_time = self.game_time;
            let state = menu.loom_state();
            if state.last_sound_time != game_time {
                state.last_sound_time = game_time;
                self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:ui.loom.take_result"), SoundSource::Blocks, 1.0, 1.0);
            }
        }
    }

    pub fn loom_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        if menu.access.is_some() {
            self.clear_container(menu, player, INPUT_CONTAINER);
        }
    }

    pub fn loom_menu_quick_move_stack(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, stack: &mut ItemStack) -> bool {
        if index == RESULT_SLOT {
            self.move_item_stack_to(menu, player, stack, INV_SLOT_START, USE_ROW_SLOT_END, true)
        } else if index != DYE_SLOT && index != BANNER_SLOT && index != PATTERN_SLOT {
            if is_banner_item(stack.get_item()) {
                self.move_item_stack_to(menu, player, stack, BANNER_SLOT, BANNER_SLOT + 1, false)
            } else if is_dye_item(stack) {
                self.move_item_stack_to(menu, player, stack, DYE_SLOT, DYE_SLOT + 1, false)
            } else if is_pattern_item(stack) {
                self.move_item_stack_to(menu, player, stack, PATTERN_SLOT, PATTERN_SLOT + 1, false)
            } else if (INV_SLOT_START..INV_SLOT_END).contains(&index) {
                self.move_item_stack_to(menu, player, stack, USE_ROW_SLOT_START, USE_ROW_SLOT_END, false)
            } else {
                !((USE_ROW_SLOT_START..USE_ROW_SLOT_END).contains(&index) && !self.move_item_stack_to(menu, player, stack, INV_SLOT_START, INV_SLOT_END, false))
            }
        } else {
            self.move_item_stack_to(menu, player, stack, INV_SLOT_START, USE_ROW_SLOT_END, false)
        }
    }
}
