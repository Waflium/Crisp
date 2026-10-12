use crisp_core::BlockPos;

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::item_combiner_menu::{INPUT_SLOTS, RESULT_SLOTS};
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_SMITHING_TABLE_USED;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::crafting::recipe_property_set::{SMITHING_ADDITION, SMITHING_BASE, SMITHING_TEMPLATE};
use crate::world::item::crafting::smithing_recipe_input::SmithingRecipeInput;

pub const TEMPLATE_SLOT: usize = 0;
pub const BASE_SLOT: usize = 1;
pub const ADDITIONAL_SLOT: usize = 2;
pub const RESULT_SLOT: usize = 3;

#[derive(Default)]
pub struct SmithingMenu {
    has_recipe_error: i32,
}

impl SmithingMenu {
    pub fn get_has_recipe_error(&self) -> i32 {
        self.has_recipe_error
    }
}

pub fn may_place(slot: usize, item_stack: &ItemStack) -> bool {
    let recipes = RecipeManager::get();
    match slot {
        TEMPLATE_SLOT => recipes.property_set(SMITHING_TEMPLATE).test(item_stack),
        BASE_SLOT => recipes.property_set(SMITHING_BASE).test(item_stack),
        _ => recipes.property_set(SMITHING_ADDITION).test(item_stack),
    }
}

impl AbstractContainerMenu {
    pub fn smithing(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Smithing, None);
        menu.access = Some(pos);
        menu.smithing = Some(SmithingMenu::default());
        menu
    }

    fn create_recipe_input(&self) -> SmithingRecipeInput {
        let input_slots = &self.containers[INPUT_SLOTS];
        SmithingRecipeInput { template: input_slots.get_item(0).clone(), base: input_slots.get_item(1).clone(), addition: input_slots.get_item(2).clone() }
    }
}

impl ServerLevel {
    pub fn smithing_menu_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, carried: &mut ItemStack) {
        let count = carried.count;
        carried.on_crafted_by(self, player, count);
        let input_slots = &menu.containers[INPUT_SLOTS];
        let relevant_items = [input_slots.get_item(0).clone(), input_slots.get_item(1).clone(), input_slots.get_item(2).clone()];
        menu.containers[RESULT_SLOTS].award_used_recipes(self, player, &relevant_items);
        self.shrink_stack_in_slot(menu, player, 0);
        self.shrink_stack_in_slot(menu, player, 1);
        self.shrink_stack_in_slot(menu, player, 2);
        if let Some(pos) = menu.access {
            self.level_event(None, SOUND_SMITHING_TABLE_USED, pos, 0);
        }
    }

    fn shrink_stack_in_slot(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, slot: usize) {
        let mut stack = menu.containers[INPUT_SLOTS].get_item(slot).clone();
        if !stack.is_empty() {
            stack.shrink(1);
            self.menu_container_set_item(menu, player, INPUT_SLOTS, slot, stack);
        }
    }

    pub fn smithing_menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, container: usize) {
        self.item_combiner_menu_slots_changed(menu, player, container);
        let has_recipe_error = self.slot_has_item(menu, player, 0) && self.slot_has_item(menu, player, 1) && self.slot_has_item(menu, player, 2) && !self.slot_has_item(menu, player, RESULT_SLOT);
        if let Some(smithing) = menu.smithing.as_mut() {
            smithing.has_recipe_error = if has_recipe_error { 1 } else { 0 };
        }
    }

    pub fn smithing_menu_create_result(&mut self, menu: &mut AbstractContainerMenu, _player: EntityId) {
        let input = menu.create_recipe_input();
        match RecipeManager::get().get_smithing_recipe_for(&input, None) {
            Some(recipe) => {
                let result = recipe.smithing().expect("a smithing recipe").assemble(&input);
                menu.containers[RESULT_SLOTS].result().set_recipe_used(Some(recipe.id));
                *menu.containers[RESULT_SLOTS].get_item_mut(0) = result;
            }
            None => {
                menu.containers[RESULT_SLOTS].result().set_recipe_used(None);
                *menu.containers[RESULT_SLOTS].get_item_mut(0) = ItemStack::empty();
            }
        }
    }

    pub fn smithing_menu_can_move_into_input_slots(&mut self, menu: &AbstractContainerMenu, player: EntityId, stack: &ItemStack) -> bool {
        let recipes = RecipeManager::get();
        if recipes.property_set(SMITHING_TEMPLATE).test(stack) && !self.slot_has_item(menu, player, 0) {
            return true;
        }
        if recipes.property_set(SMITHING_BASE).test(stack) && !self.slot_has_item(menu, player, 1) { true } else { recipes.property_set(SMITHING_ADDITION).test(stack) && !self.slot_has_item(menu, player, 2) }
    }
}
