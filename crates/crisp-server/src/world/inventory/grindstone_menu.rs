use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::anvil_menu::calculate_increased_repair_cost;
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_GRINDSTONE_USED;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::codec;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::{get_enchantments_for_crafting, has_any_enchantments, update_enchantments};

pub const INPUT_SLOT: usize = 0;
pub const ADDITIONAL_SLOT: usize = 1;
pub const RESULT_SLOT: usize = 2;
pub const INV_SLOT_START: usize = 3;
pub const INV_SLOT_END: usize = 30;
pub const USE_ROW_SLOT_START: usize = 30;
pub const USE_ROW_SLOT_END: usize = 39;
pub const REPAIR_SLOTS: usize = 0;
pub const RESULT_SLOTS: usize = 1;

fn is_curse(enchantment: &str) -> bool {
    codec::registry_id("minecraft:enchantment", enchantment).is_ok_and(|id| crisp_data::tag("minecraft:enchantment", "minecraft:curse").contains(&id))
}

pub fn may_place(item_stack: &ItemStack) -> bool {
    item_stack.is_damageable_item() || has_any_enchantments(item_stack)
}

fn get_experience_from_item(item: &ItemStack) -> i32 {
    let mut amount = 0;
    for (enchant, lvl) in &get_enchantments_for_crafting(item).enchantments {
        if !is_curse(enchant) {
            amount += Enchantment::by_id(enchant).map_or(0, |enchantment| enchantment.min_cost(*lvl));
        }
    }
    amount
}

fn compute_result(input: &ItemStack, additional: &ItemStack) -> ItemStack {
    let has_an_item = !input.is_empty() || !additional.is_empty();
    if !has_an_item {
        return ItemStack::empty();
    }
    if input.count > 1 || additional.count > 1 {
        return ItemStack::empty();
    }
    let has_both_items = !input.is_empty() && !additional.is_empty();
    if !has_both_items {
        let item = if !input.is_empty() { input } else { additional };
        return if !has_any_enchantments(item) { ItemStack::empty() } else { remove_non_curses_from(item.copy()) };
    }
    merge_items(input, additional)
}

fn merge_items(input: &ItemStack, additional: &ItemStack) -> ItemStack {
    if !input.is(additional.get_item()) {
        return ItemStack::empty();
    }
    let durability = input.max_damage().max(additional.max_damage());
    let remaining1 = input.max_damage() - input.damage_value();
    let remaining2 = additional.max_damage() - additional.damage_value();
    let remaining = remaining1 + remaining2 + durability * 5 / 100;
    let mut count = 1;
    if !input.is_damageable_item() {
        if input.max_stack_size() < 2 || !ItemStack::matches(input, additional) {
            return ItemStack::empty();
        }
        count = 2;
    }
    let mut new_item = input.copy_with_count(count);
    if new_item.is_damageable_item() {
        new_item.set(dc::MAX_DAMAGE, Some(Nbt::Int(durability)));
        new_item.set_damage_value((durability - remaining).max(0));
    }
    merge_enchants_from(&mut new_item, additional);
    remove_non_curses_from(new_item)
}

fn merge_enchants_from(target: &mut ItemStack, source: &ItemStack) {
    update_enchantments(target, |new_enchantments| {
        for (enchant, level) in &get_enchantments_for_crafting(source).enchantments {
            if !is_curse(enchant) || new_enchantments.level(enchant) == 0 {
                new_enchantments.upgrade(enchant, *level);
            }
        }
    });
}

fn remove_non_curses_from(mut item: ItemStack) -> ItemStack {
    let new_enchantments = update_enchantments(&mut item, |enchantments| enchantments.enchantments.retain(|(enchantment, _)| is_curse(enchantment)));
    if item.is(items::ENCHANTED_BOOK) && new_enchantments.is_empty() {
        item = item.transmute_copy(items::BOOK, item.count);
    }
    let mut repair_cost = 0;
    for _ in 0..new_enchantments.enchantments.len() {
        repair_cost = calculate_increased_repair_cost(repair_cost);
    }
    item.set(dc::REPAIR_COST, Some(Nbt::Int(repair_cost)));
    item
}

impl AbstractContainerMenu {
    pub fn grindstone(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Grindstone, None);
        menu.access = Some(pos);
        menu
    }
}

impl ServerLevel {
    pub fn grindstone_result_slot_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        if let Some(pos) = menu.access {
            let experience_amount = self.grindstone_get_experience_amount(menu);
            self.award_experience(Vec3::at_center_of(pos), experience_amount);
            self.level_event(None, SOUND_GRINDSTONE_USED, pos, 0);
        }
        self.menu_container_set_item(menu, player, REPAIR_SLOTS, INPUT_SLOT, ItemStack::empty());
        self.menu_container_set_item(menu, player, REPAIR_SLOTS, ADDITIONAL_SLOT, ItemStack::empty());
    }

    fn grindstone_get_experience_amount(&mut self, menu: &AbstractContainerMenu) -> i32 {
        let mut amount = 0;
        amount += get_experience_from_item(menu.containers[REPAIR_SLOTS].get_item(INPUT_SLOT));
        amount += get_experience_from_item(menu.containers[REPAIR_SLOTS].get_item(ADDITIONAL_SLOT));
        if amount > 0 {
            let half_amount = (amount as f64 / 2.0).ceil() as i32;
            half_amount + self.random.next_int_bounded(half_amount)
        } else {
            0
        }
    }

    pub fn grindstone_menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, container: usize) {
        self.menu_broadcast_changes(menu, player);
        if container == REPAIR_SLOTS {
            self.grindstone_menu_create_result(menu, player);
        }
    }

    fn grindstone_menu_create_result(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let result = compute_result(menu.containers[REPAIR_SLOTS].get_item(INPUT_SLOT), menu.containers[REPAIR_SLOTS].get_item(ADDITIONAL_SLOT));
        *menu.containers[RESULT_SLOTS].get_item_mut(0) = result;
        self.menu_broadcast_changes(menu, player);
    }

    pub fn grindstone_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        if menu.access.is_some() {
            self.clear_container(menu, player, REPAIR_SLOTS);
        }
    }

    pub fn grindstone_menu_quick_move_stack(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, item: &mut ItemStack) -> bool {
        let input_is_empty = menu.containers[REPAIR_SLOTS].get_item(INPUT_SLOT).is_empty();
        let additional_is_empty = menu.containers[REPAIR_SLOTS].get_item(ADDITIONAL_SLOT).is_empty();
        if index == RESULT_SLOT {
            self.move_item_stack_to(menu, player, item, INV_SLOT_START, USE_ROW_SLOT_END, true)
        } else if index != INPUT_SLOT && index != ADDITIONAL_SLOT {
            if !input_is_empty && !additional_is_empty {
                if (INV_SLOT_START..INV_SLOT_END).contains(&index) {
                    self.move_item_stack_to(menu, player, item, USE_ROW_SLOT_START, USE_ROW_SLOT_END, false)
                } else {
                    !((USE_ROW_SLOT_START..USE_ROW_SLOT_END).contains(&index) && !self.move_item_stack_to(menu, player, item, INV_SLOT_START, INV_SLOT_END, false))
                }
            } else {
                self.move_item_stack_to(menu, player, item, INPUT_SLOT, RESULT_SLOT, false)
            }
        } else {
            self.move_item_stack_to(menu, player, item, INV_SLOT_START, USE_ROW_SLOT_END, false)
        }
    }
}
