use crisp_data::Item;

use super::place_recipe_helper;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::player::stacked_item_contents::StackedItemContents;
use crate::world::inventory::abstract_container_menu::AbstractContainerMenu;
use crate::world::inventory::recipe_book_menu::PostPlaceAction;
use crate::world::item::ItemStack;
use crate::world::item::crafting::recipe_holder::RecipeHolder;

pub trait CraftingMenuAccess {
    fn fill_craft_slots_stacked_contents(&self, level: &mut ServerLevel, menu: &AbstractContainerMenu, stacked_contents: &mut StackedItemContents);

    fn clear_crafting_content(&self, level: &mut ServerLevel, menu: &mut AbstractContainerMenu, player: EntityId);

    fn recipe_matches(&self, level: &mut ServerLevel, menu: &AbstractContainerMenu, recipe: &RecipeHolder) -> Option<bool>;
}

pub struct ServerPlaceRecipe<'a, A: CraftingMenuAccess> {
    player: EntityId,
    menu: &'a A,
    use_max_items: bool,
    grid_width: i32,
    grid_height: i32,
    input_grid_slots: &'a [usize],
    slots_to_clear: &'a [usize],
}

#[allow(clippy::too_many_arguments)]
pub fn place_recipe<A: CraftingMenuAccess>(
    level: &mut ServerLevel,
    container_menu: &mut AbstractContainerMenu,
    menu: &A,
    grid_width: i32,
    grid_height: i32,
    input_grid_slots: &[usize],
    slots_to_clear: &[usize],
    player: EntityId,
    recipe: &RecipeHolder,
    use_max_items: bool,
    allow_dropping_items_to_clear: bool,
) -> Option<PostPlaceAction> {
    let placer = ServerPlaceRecipe { player, menu, use_max_items, grid_width, grid_height, input_grid_slots, slots_to_clear };
    if !allow_dropping_items_to_clear && !placer.test_clear_grid(level, container_menu) {
        return Some(PostPlaceAction::Nothing);
    }
    let mut available_items = StackedItemContents::default();
    level.player(player).inventory.fill_stacked_contents(&mut available_items);
    menu.fill_craft_slots_stacked_contents(level, container_menu, &mut available_items);
    placer.try_place_recipe(level, container_menu, recipe, &mut available_items)
}

fn clamp_to_max_stack_size(mut value: i32, items: &[Item]) -> i32 {
    for item in items {
        value = value.min(item.info().max_stack_size as i32);
    }
    value
}

impl<A: CraftingMenuAccess> ServerPlaceRecipe<'_, A> {
    fn try_place_recipe(&self, level: &mut ServerLevel, container_menu: &mut AbstractContainerMenu, recipe: &RecipeHolder, available_items: &mut StackedItemContents) -> Option<PostPlaceAction> {
        if available_items.can_craft(&recipe.value, None) {
            self.place_recipe(level, container_menu, recipe, available_items)?;
            Some(PostPlaceAction::Nothing)
        } else {
            self.clear_grid(level, container_menu);
            Some(PostPlaceAction::PlaceGhostRecipe)
        }
    }

    fn clear_grid(&self, level: &mut ServerLevel, container_menu: &mut AbstractContainerMenu) {
        for &slot in self.slots_to_clear {
            let mut item_stack_copy = level.slot_get(container_menu, self.player, slot).copy();
            level.place_item_back_in_inventory(self.player, &mut item_stack_copy, false);
            level.slot_set(container_menu, self.player, slot, item_stack_copy);
        }
        self.menu.clear_crafting_content(level, container_menu, self.player);
    }

    fn place_recipe(&self, level: &mut ServerLevel, container_menu: &mut AbstractContainerMenu, recipe: &RecipeHolder, available_items: &mut StackedItemContents) -> Option<()> {
        let recipe_matches_placed = self.menu.recipe_matches(level, container_menu, recipe)?;
        let biggest_craftable_stack = available_items.get_biggest_craftable_stack(&recipe.value, i32::MAX, None);
        if recipe_matches_placed {
            for &input_slot in self.input_grid_slots {
                let item_stack = level.slot_get(container_menu, self.player, input_slot);
                if !item_stack.is_empty() && biggest_craftable_stack.min(item_stack.max_stack_size()) < item_stack.count + 1 {
                    return Some(());
                }
            }
        }
        let amount_to_craft = self.calculate_amount_to_craft(level, container_menu, biggest_craftable_stack, recipe_matches_placed);
        let mut items_used_per_ingredient = Vec::new();
        if available_items.can_craft_amount(&recipe.value, amount_to_craft, Some(&mut |item| items_used_per_ingredient.push(item))) {
            let adjusted_amount_to_craft = clamp_to_max_stack_size(amount_to_craft, &items_used_per_ingredient);
            if adjusted_amount_to_craft != amount_to_craft {
                items_used_per_ingredient.clear();
                if !available_items.can_craft_amount(&recipe.value, adjusted_amount_to_craft, Some(&mut |item| items_used_per_ingredient.push(item))) {
                    return Some(());
                }
            }
            self.clear_grid(level, container_menu);
            let placement_info = recipe.value.placement_info();
            place_recipe_helper::place_recipe(self.grid_width, self.grid_height, &recipe.value, placement_info.slots_to_ingredient_index().iter().copied(), |ingredient_index, grid_index, _, _| {
                if ingredient_index != -1 {
                    let target_grid_slot = self.input_grid_slots[grid_index as usize];
                    let item_used = items_used_per_ingredient[ingredient_index as usize];
                    let mut remaining_count = adjusted_amount_to_craft;
                    while remaining_count > 0 {
                        remaining_count = self.move_item_to_grid(level, container_menu, target_grid_slot, item_used, remaining_count);
                        if remaining_count == -1 {
                            return;
                        }
                    }
                }
            });
        }
        Some(())
    }

    fn calculate_amount_to_craft(&self, level: &mut ServerLevel, container_menu: &AbstractContainerMenu, biggest_craftable_stack: i32, recipe_matches_placed: bool) -> i32 {
        if self.use_max_items {
            return biggest_craftable_stack;
        }
        if recipe_matches_placed {
            let mut smallest_stack_size = i32::MAX;
            for &input_slot in self.input_grid_slots {
                let item_stack = level.slot_get(container_menu, self.player, input_slot);
                if !item_stack.is_empty() && smallest_stack_size > item_stack.count {
                    smallest_stack_size = item_stack.count;
                }
            }
            if smallest_stack_size != i32::MAX {
                smallest_stack_size += 1;
            }
            smallest_stack_size
        } else {
            1
        }
    }

    fn move_item_to_grid(&self, level: &mut ServerLevel, container_menu: &mut AbstractContainerMenu, target_slot: usize, item_in_inventory: Item, count: i32) -> i32 {
        let mut item_in_target_slot = level.slot_get(container_menu, self.player, target_slot);
        let inventory = &mut level.player_mut(self.player).inventory;
        let Some(inventory_slot_id) = inventory.find_slot_matching_crafting_ingredient(item_in_inventory, &item_in_target_slot) else { return -1 };
        let inventory_item = inventory.get_item(inventory_slot_id);
        let taken_stack = if count < inventory_item.count { inventory.remove_item(inventory_slot_id, count) } else { inventory.remove_item_no_update(inventory_slot_id) };
        let taken_count = taken_stack.count;
        if item_in_target_slot.is_empty() {
            level.slot_set(container_menu, self.player, target_slot, taken_stack);
        } else {
            item_in_target_slot.grow(taken_count);
            level.slot_put(container_menu, self.player, target_slot, item_in_target_slot);
        }
        count - taken_count
    }

    fn test_clear_grid(&self, level: &mut ServerLevel, container_menu: &AbstractContainerMenu) -> bool {
        let mut free_slots: Vec<ItemStack> = Vec::new();
        let free_slots_in_inventory = self.get_amount_of_free_slots_in_inventory(level);
        for &input_slot in self.input_grid_slots {
            let mut item_stack = level.slot_get(container_menu, self.player, input_slot).copy();
            if !item_stack.is_empty() {
                let slot_id = level.player(self.player).inventory.slot_with_remaining_space(&item_stack);
                if slot_id.is_none() && free_slots.len() <= free_slots_in_inventory {
                    for item_stack_in_list in &mut free_slots {
                        if ItemStack::is_same_item(item_stack_in_list, &item_stack)
                            && item_stack_in_list.count != item_stack_in_list.max_stack_size()
                            && item_stack_in_list.count + item_stack.count <= item_stack_in_list.max_stack_size()
                        {
                            item_stack_in_list.grow(item_stack.count);
                            item_stack.count = 0;
                            break;
                        }
                    }
                    if !item_stack.is_empty() {
                        if free_slots.len() >= free_slots_in_inventory {
                            return false;
                        }
                        free_slots.push(item_stack);
                    }
                } else if slot_id.is_none() {
                    return false;
                }
            }
        }
        true
    }

    fn get_amount_of_free_slots_in_inventory(&self, level: &ServerLevel) -> usize {
        let inventory = &level.player(self.player).inventory;
        (0..36).filter(|&i| inventory.get_item(i).is_empty()).count()
    }
}
