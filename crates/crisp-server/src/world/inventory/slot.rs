use crisp_data::{EquipmentSlot, items};

use super::abstract_container_menu::AbstractContainerMenu;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::item::component as dc;
use crate::world::item::crafting::potion_ingredient::PotionIngredient;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::crafting::recipe_property_set::BREWING_REAGENTS;
use crate::world::item::{ItemStack, inventory_slot};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotContainer {
    Inventory,
    Container(ContainerRef),
    Menu(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotKind {
    Normal,
    ShulkerBox,
    FurnaceFuel,
    FurnaceResult { remove_count: i32 },
    BrewingStandPotion,
    BrewingStandIngredients,
    BrewingStandFuel,
    BeaconPayment,
    EnchantmentItem,
    EnchantmentLapis,
    Result { remove_count: i32, craft_slots: usize },
    Armor(EquipmentSlot),
    Offhand,
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub container: SlotContainer,
    pub slot: usize,
    pub index: usize,
    pub kind: SlotKind,
}

impl Slot {
    pub fn new(container: SlotContainer, slot: usize, kind: SlotKind) -> Slot {
        Slot { container, slot, index: 0, kind }
    }
}

pub fn is_bucket(stack: &ItemStack) -> bool {
    stack.is(items::BUCKET)
}

pub fn is_fuel(stack: &ItemStack) -> bool {
    stack.has(dc::COOKING_FUEL)
}

pub fn brewing_stand_fuel_may_place_item(stack: &ItemStack) -> bool {
    stack.has(dc::BREWING_FUEL)
}

impl ServerLevel {
    pub fn slot_get(&mut self, menu: &AbstractContainerMenu, player: EntityId, index: usize) -> ItemStack {
        let slot = &menu.slots[index];
        match slot.container {
            SlotContainer::Inventory => self.player(player).inventory.slots[inventory_slot(slot.slot)].clone().unwrap_or_else(ItemStack::empty),
            SlotContainer::Container(c) => self.get_item(c, slot.slot),
            SlotContainer::Menu(c) => menu.containers[c].get_item(slot.slot).clone(),
        }
    }

    pub fn slot_put(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, stack: ItemStack) {
        let slot = &menu.slots[index];
        match slot.container {
            SlotContainer::Inventory => self.player_mut(player).inventory.slots[inventory_slot(slot.slot)] = (!stack.is_empty()).then_some(stack),
            SlotContainer::Container(c) => self.put_item(c, slot.slot, stack),
            SlotContainer::Menu(c) => *menu.containers[c].get_item_mut(slot.slot) = stack,
        }
    }

    pub fn slot_has_item(&mut self, menu: &AbstractContainerMenu, player: EntityId, index: usize) -> bool {
        !self.slot_get(menu, player, index).is_empty()
    }

    pub fn slot_set(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, stack: ItemStack) {
        let slot = &menu.slots[index];
        match slot.container {
            SlotContainer::Inventory => self.slot_put(menu, player, index, stack),
            SlotContainer::Container(c) => self.set_item(c, slot.slot, stack),
            SlotContainer::Menu(c) => {
                let container_slot = slot.slot;
                self.menu_container_set_item(menu, player, c, container_slot, stack);
            }
        }
        self.slot_set_changed(menu, player, index);
    }

    pub fn slot_set_by_player(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, stack: ItemStack) {
        let previous = self.slot_get(menu, player, index);
        match menu.slots[index].kind {
            SlotKind::Armor(equipment_slot) => self.on_equip_item(player, equipment_slot, &previous, &stack),
            SlotKind::Offhand => self.on_equip_item(player, EquipmentSlot::Offhand, &previous, &stack),
            _ => {}
        }
        self.slot_set(menu, player, index, stack);
    }

    pub fn slot_set_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize) {
        match menu.slots[index].container {
            SlotContainer::Inventory => {}
            SlotContainer::Container(c) => self.container_set_changed(c),
            SlotContainer::Menu(c) => self.menu_container_set_changed(menu, player, c),
        }
    }

    pub fn slot_max_stack_size(&self, menu: &AbstractContainerMenu, index: usize, stack: &ItemStack) -> i32 {
        let slot = &menu.slots[index];
        match slot.kind {
            SlotKind::FurnaceFuel if is_bucket(stack) => return 1,
            SlotKind::BrewingStandPotion | SlotKind::BeaconPayment | SlotKind::EnchantmentItem | SlotKind::Armor(_) => return stack.max_stack_size().min(1),
            _ => {}
        }
        let container_max_stack_size = match slot.container {
            SlotContainer::Menu(c) => menu.containers[c].get_max_stack_size(),
            _ => 99,
        };
        stack.max_stack_size().min(container_max_stack_size)
    }

    pub fn slot_may_place(&self, menu: &AbstractContainerMenu, player: EntityId, index: usize, stack: &ItemStack) -> bool {
        match menu.slots[index].kind {
            SlotKind::Normal | SlotKind::Offhand | SlotKind::EnchantmentItem => true,
            SlotKind::ShulkerBox => !stack.get_item().info().block.is_some_and(|b| b.is_instance_of("ShulkerBoxBlock")),
            SlotKind::FurnaceFuel => is_fuel(stack) || is_bucket(stack),
            SlotKind::FurnaceResult { .. } | SlotKind::Result { .. } => false,
            SlotKind::BrewingStandPotion => PotionIngredient::is_potion_input(stack),
            SlotKind::BrewingStandIngredients => RecipeManager::get().property_set(BREWING_REAGENTS).test(stack),
            SlotKind::BrewingStandFuel => brewing_stand_fuel_may_place_item(stack),
            SlotKind::BeaconPayment => stack.get_item().is(&crisp_data::item_tags::BEACON_PAYMENT_ITEMS),
            SlotKind::EnchantmentLapis => stack.is(items::LAPIS_LAZULI),
            SlotKind::Armor(equipment_slot) => self.is_equippable_in_slot(player, stack, equipment_slot),
        }
    }

    pub fn slot_may_pickup(&mut self, menu: &AbstractContainerMenu, player: EntityId, index: usize) -> bool {
        if let SlotKind::Armor(_) = menu.slots[index].kind {
            let item_stack = self.slot_get(menu, player, index);
            if !item_stack.is_empty() && !self.player(player).game_mode.game_type.is_creative() && crate::world::item::enchantment::enchantment_helper::has_prevent_armor_change(&item_stack) {
                return false;
            }
        }
        true
    }

    pub fn slot_remove(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, amount: i32) -> ItemStack {
        if let SlotKind::FurnaceResult { .. } | SlotKind::Result { .. } = menu.slots[index].kind {
            let current = self.slot_get(menu, player, index);
            if !current.is_empty()
                && let SlotKind::FurnaceResult { remove_count } | SlotKind::Result { remove_count, .. } = &mut menu.slots[index].kind
            {
                *remove_count += amount.min(current.count);
            }
        }
        let slot = &menu.slots[index];
        match slot.container {
            SlotContainer::Inventory => {
                let i = inventory_slot(slot.slot);
                let p = self.player_mut(player);
                match &mut p.inventory.slots[i] {
                    Some(stack) if !stack.is_empty() && amount > 0 => {
                        let result = stack.split(amount);
                        if stack.is_empty() {
                            p.inventory.slots[i] = None;
                        }
                        result
                    }
                    _ => ItemStack::empty(),
                }
            }
            SlotContainer::Container(c) => self.remove_item(c, slot.slot, amount),
            SlotContainer::Menu(c) => {
                let container_slot = slot.slot;
                self.menu_container_remove_item(menu, player, c, container_slot, amount)
            }
        }
    }

    pub fn slot_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, carried: &ItemStack) {
        match menu.slots[index].kind {
            SlotKind::FurnaceResult { .. } => {
                self.check_take_achievements(menu, player, index, carried);
                self.slot_set_changed(menu, player, index);
            }
            SlotKind::Result { .. } => self.result_slot_on_take(menu, player, index),
            _ => self.slot_set_changed(menu, player, index),
        }
    }

    pub fn slot_on_quick_craft(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, picked: &ItemStack, original: &ItemStack) {
        let count = original.count - picked.count;
        if count > 0
            && let SlotKind::FurnaceResult { remove_count } | SlotKind::Result { remove_count, .. } = &mut menu.slots[index].kind
        {
            *remove_count += count;
            self.check_take_achievements(menu, player, index, original);
        }
    }

    pub fn slot_on_swap_craft(&mut self, menu: &mut AbstractContainerMenu, index: usize, count: i32) {
        if let SlotKind::Result { remove_count, .. } = &mut menu.slots[index].kind {
            *remove_count += count;
        }
    }

    fn check_take_achievements(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, _carried: &ItemStack) {
        match menu.slots[index].kind {
            SlotKind::FurnaceResult { .. } => {
                if let SlotContainer::Container(c) = menu.slots[index].container
                    && let Some(pos) = c.single_block()
                {
                    self.award_used_recipes_and_pop_experience(pos, player);
                }
            }
            SlotKind::Result { .. } => {
                if let SlotContainer::Menu(c) = menu.slots[index].container {
                    menu.containers[c].award_used_recipes();
                }
            }
            _ => {}
        }
        if let SlotKind::FurnaceResult { remove_count } | SlotKind::Result { remove_count, .. } = &mut menu.slots[index].kind {
            *remove_count = 0;
        }
    }

    pub fn slot_allow_modification(&mut self, menu: &AbstractContainerMenu, player: EntityId, index: usize) -> bool {
        let item = self.slot_get(menu, player, index);
        self.slot_may_pickup(menu, player, index) && self.slot_may_place(menu, player, index, &item)
    }

    pub fn slot_try_remove(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, amount: i32, max_amount: i32) -> Option<ItemStack> {
        if !self.slot_may_pickup(menu, player, index) {
            return None;
        }
        if !self.slot_allow_modification(menu, player, index) && max_amount < self.slot_get(menu, player, index).count {
            return None;
        }
        let amount = amount.min(max_amount);
        let result = self.slot_remove(menu, player, index, amount);
        if result.is_empty() {
            return None;
        }
        if self.slot_get(menu, player, index).is_empty() {
            self.slot_set_by_player(menu, player, index, ItemStack::empty());
        }
        Some(result)
    }

    pub fn slot_safe_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, amount: i32, max_amount: i32) -> ItemStack {
        let result = self.slot_try_remove(menu, player, index, amount, max_amount);
        if let Some(taken) = &result {
            self.slot_on_take(menu, player, index, taken);
        }
        result.unwrap_or_else(ItemStack::empty)
    }

    pub fn slot_safe_insert(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, mut input: ItemStack, amount: i32) -> ItemStack {
        if input.is_empty() || !self.slot_may_place(menu, player, index, &input) {
            return input;
        }
        let mut slot_stack = self.slot_get(menu, player, index);
        let transferable = amount.min(input.count).min(self.slot_max_stack_size(menu, index, &input) - slot_stack.count);
        if transferable <= 0 {
            return input;
        }
        if slot_stack.is_empty() {
            let piece = input.split(transferable);
            self.slot_set_by_player(menu, player, index, piece);
        } else if ItemStack::is_same_item_same_components(&slot_stack, &input) {
            input.shrink(transferable);
            slot_stack.grow(transferable);
            self.slot_set_by_player(menu, player, index, slot_stack);
        }
        input
    }

    pub fn slot_safe_clone(&mut self, menu: &AbstractContainerMenu, player: EntityId, index: usize) -> ItemStack {
        let item = self.slot_get(menu, player, index);
        let max = item.max_stack_size();
        item.copy_with_count(max)
    }
}
