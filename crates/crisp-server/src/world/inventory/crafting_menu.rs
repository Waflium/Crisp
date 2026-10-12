use crisp_core::BlockPos;

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::slot::{SlotContainer, SlotKind};
use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::crafting::crafting_input::CraftingInput;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;

pub const RESULT_SLOT: usize = 0;
pub const CRAFT_SLOTS: usize = 0;
pub const RESULT_SLOTS: usize = 1;
const INPUT_GRID_SLOTS: [usize; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];

impl AbstractContainerMenu {
    pub fn crafting(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Crafting, None);
        menu.access = Some(pos);
        menu
    }

    pub fn crafting_menu_begin_placing_recipe(&mut self) {
        self.placing_recipe = true;
    }

    pub fn crafting_menu_get_input_grid_slots(&self) -> &'static [usize] {
        &INPUT_GRID_SLOTS
    }
}

fn copy_all_input_items(input: &CraftingInput) -> Vec<ItemStack> {
    input.items().to_vec()
}

impl ServerLevel {
    pub fn slot_changed_crafting_grid(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, recipe_hint: Option<&str>) {
        let mut input = menu.containers[CRAFT_SLOTS].crafting().as_craft_input();
        let mut result = ItemStack::empty();
        if let Some(recipe_holder) = RecipeManager::get().get_crafting_recipe_for(&mut input, self, recipe_hint)
            && menu.containers[RESULT_SLOTS].result().set_recipe_used_by(self, player, recipe_holder)
        {
            let crafting_recipe = recipe_holder.crafting().expect("a crafting recipe");
            result = crafting_recipe.assemble(&input);
        }
        *menu.containers[RESULT_SLOTS].get_item_mut(0) = result.clone();
        menu.set_remote_slot(RESULT_SLOT, &result);
        let state_id = menu.increment_state_id();
        self.send_container_set_slot(player, menu.container_id, state_id, RESULT_SLOT, &result);
    }

    pub fn crafting_menu_finish_placing_recipe(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, recipe: &RecipeHolder) -> Option<()> {
        menu.placing_recipe = false;
        recipe.crafting()?;
        self.slot_changed_crafting_grid(menu, player, Some(recipe.id));
        Some(())
    }

    fn get_remaining_items(&mut self, input: &mut CraftingInput) -> Vec<ItemStack> {
        match RecipeManager::get().get_crafting_recipe_for(input, self, None) {
            Some(recipe) => recipe.crafting().expect("a crafting recipe").get_remaining_items(input),
            None => copy_all_input_items(input),
        }
    }

    pub fn result_slot_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize) {
        let SlotKind::Result { craft_slots, .. } = menu.slots[index].kind else { return };
        if let SlotContainer::Menu(c) = menu.slots[index].container {
            menu.containers[c].award_used_recipes(self, player);
        }
        if let SlotKind::Result { remove_count, .. } = &mut menu.slots[index].kind {
            *remove_count = 0;
        }
        let positioned_recipe = menu.containers[craft_slots].crafting().as_positioned_craft_input();
        let mut input = positioned_recipe.input;
        let (recipe_left, recipe_top) = (positioned_recipe.left, positioned_recipe.top);
        let remaining = self.get_remaining_items(&mut input);
        let width = menu.containers[craft_slots].crafting().get_width();
        for y in 0..input.height() {
            for x in 0..input.width() {
                let slot = x + recipe_left + (y + recipe_top) * width;
                let mut item_stack = menu.containers[craft_slots].get_item(slot).clone();
                let mut replacement = remaining[x + y * input.width()].clone();
                if !item_stack.is_empty() {
                    self.menu_container_remove_item(menu, player, craft_slots, slot, 1);
                    item_stack = menu.containers[craft_slots].get_item(slot).clone();
                }
                if !replacement.is_empty() {
                    if item_stack.is_empty() {
                        self.menu_container_set_item(menu, player, craft_slots, slot, replacement);
                    } else if ItemStack::is_same_item_same_components(&item_stack, &replacement) {
                        replacement.grow(item_stack.count);
                        self.menu_container_set_item(menu, player, craft_slots, slot, replacement);
                    } else {
                        let infinite_materials = self.player(player).abilities.instabuild;
                        self.player_mut(player).inventory.add_stack(&mut replacement, infinite_materials);
                        if !replacement.is_empty() {
                            self.player_drop(player, replacement, false, Prediction::Predicted);
                        }
                    }
                }
            }
        }
    }
}
