use crisp_data::items;

use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::item::component as dc;
use crate::world::item::crafting::potion_ingredient::PotionIngredient;
use crate::world::item::crafting::recipe_property_set::{BREWING_REAGENTS, RecipePropertySet};
use crate::world::item::{ItemStack, inventory_slot};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotContainer {
    Inventory,
    Container(ContainerRef),
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
    pub fn slot_get(&mut self, player: EntityId, slot: &Slot) -> ItemStack {
        match slot.container {
            SlotContainer::Inventory => self.player(player).inventory.slots[inventory_slot(slot.slot)].clone().unwrap_or_else(ItemStack::empty),
            SlotContainer::Container(c) => self.get_item(c, slot.slot),
        }
    }

    pub fn slot_put(&mut self, player: EntityId, slot: &Slot, stack: ItemStack) {
        match slot.container {
            SlotContainer::Inventory => self.player_mut(player).inventory.slots[inventory_slot(slot.slot)] = (!stack.is_empty()).then_some(stack),
            SlotContainer::Container(c) => self.put_item(c, slot.slot, stack),
        }
    }

    pub fn slot_has_item(&mut self, player: EntityId, slot: &Slot) -> bool {
        !self.slot_get(player, slot).is_empty()
    }

    pub fn slot_set(&mut self, player: EntityId, slot: &Slot, stack: ItemStack) {
        match slot.container {
            SlotContainer::Inventory => self.slot_put(player, slot, stack),
            SlotContainer::Container(c) => self.set_item(c, slot.slot, stack),
        }
        self.slot_set_changed(slot);
    }

    pub fn slot_set_by_player(&mut self, player: EntityId, slot: &Slot, stack: ItemStack) {
        self.slot_set(player, slot, stack);
    }

    pub fn slot_set_changed(&mut self, slot: &Slot) {
        if let SlotContainer::Container(c) = slot.container {
            self.container_set_changed(c);
        }
    }

    pub fn slot_max_stack_size(&self, slot: &Slot, stack: &ItemStack) -> i32 {
        if slot.kind == SlotKind::FurnaceFuel && is_bucket(stack) {
            return 1;
        }
        if let SlotKind::BrewingStandPotion | SlotKind::BeaconPayment = slot.kind {
            return stack.max_stack_size().min(1);
        }
        stack.max_stack_size().min(99)
    }

    pub fn slot_may_place(&self, slot: &Slot, stack: &ItemStack) -> bool {
        match slot.kind {
            SlotKind::Normal => true,
            SlotKind::ShulkerBox => !stack.get_item().info().block.is_some_and(|b| b.is_instance_of("ShulkerBoxBlock")),
            SlotKind::FurnaceFuel => is_fuel(stack) || is_bucket(stack),
            SlotKind::FurnaceResult { .. } => false,
            SlotKind::BrewingStandPotion => PotionIngredient::is_potion_input(stack),
            SlotKind::BrewingStandIngredients => RecipePropertySet::get(BREWING_REAGENTS).test(stack),
            SlotKind::BrewingStandFuel => brewing_stand_fuel_may_place_item(stack),
            SlotKind::BeaconPayment => stack.get_item().is(&crisp_data::item_tags::BEACON_PAYMENT_ITEMS),
        }
    }

    pub fn slot_may_pickup(&self, _player: EntityId, _slot: &Slot) -> bool {
        true
    }

    pub fn slot_remove(&mut self, player: EntityId, slot: &mut Slot, amount: i32) -> ItemStack {
        if let SlotKind::FurnaceResult { remove_count } = &mut slot.kind {
            let current = match slot.container {
                SlotContainer::Container(c) => self.get_item(c, slot.slot),
                SlotContainer::Inventory => ItemStack::empty(),
            };
            if !current.is_empty() {
                *remove_count += amount.min(current.count);
            }
        }
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
        }
    }

    pub fn slot_on_take(&mut self, player: EntityId, slot: &mut Slot, carried: &ItemStack) {
        if let SlotKind::FurnaceResult { .. } = slot.kind {
            self.check_take_achievements(player, slot, carried);
        }
        self.slot_set_changed(slot);
    }

    pub fn slot_on_quick_craft(&mut self, player: EntityId, slot: &mut Slot, picked: &ItemStack, original: &ItemStack) {
        let count = original.count - picked.count;
        if count > 0
            && let SlotKind::FurnaceResult { remove_count } = &mut slot.kind
        {
            *remove_count += count;
            self.check_take_achievements(player, slot, original);
        }
    }

    fn check_take_achievements(&mut self, player: EntityId, slot: &mut Slot, _carried: &ItemStack) {
        if let SlotContainer::Container(c) = slot.container
            && let Some(pos) = c.single_block()
        {
            self.award_used_recipes_and_pop_experience(pos, player);
        }
        if let SlotKind::FurnaceResult { remove_count } = &mut slot.kind {
            *remove_count = 0;
        }
    }

    pub fn slot_allow_modification(&mut self, player: EntityId, slot: &Slot) -> bool {
        let item = self.slot_get(player, slot);
        self.slot_may_pickup(player, slot) && self.slot_may_place(slot, &item)
    }

    pub fn slot_try_remove(&mut self, player: EntityId, slot: &mut Slot, amount: i32, max_amount: i32) -> Option<ItemStack> {
        if !self.slot_may_pickup(player, slot) {
            return None;
        }
        if !self.slot_allow_modification(player, slot) && max_amount < self.slot_get(player, slot).count {
            return None;
        }
        let amount = amount.min(max_amount);
        let result = self.slot_remove(player, slot, amount);
        if result.is_empty() {
            return None;
        }
        if self.slot_get(player, slot).is_empty() {
            self.slot_set_by_player(player, slot, ItemStack::empty());
        }
        Some(result)
    }

    pub fn slot_safe_take(&mut self, player: EntityId, slot: &mut Slot, amount: i32, max_amount: i32) -> ItemStack {
        let result = self.slot_try_remove(player, slot, amount, max_amount);
        if let Some(taken) = &result {
            self.slot_on_take(player, slot, taken);
        }
        result.unwrap_or_else(ItemStack::empty)
    }

    pub fn slot_safe_insert(&mut self, player: EntityId, slot: &Slot, mut input: ItemStack, amount: i32) -> ItemStack {
        if input.is_empty() || !self.slot_may_place(slot, &input) {
            return input;
        }
        let mut slot_stack = self.slot_get(player, slot);
        let transferable = amount.min(input.count).min(self.slot_max_stack_size(slot, &input) - slot_stack.count);
        if transferable <= 0 {
            return input;
        }
        if slot_stack.is_empty() {
            let piece = input.split(transferable);
            self.slot_set_by_player(player, slot, piece);
        } else if ItemStack::is_same_item_same_components(&slot_stack, &input) {
            input.shrink(transferable);
            slot_stack.grow(transferable);
            self.slot_set_by_player(player, slot, slot_stack);
        }
        input
    }

    pub fn slot_safe_clone(&mut self, player: EntityId, slot: &Slot) -> ItemStack {
        let item = self.slot_get(player, slot);
        let max = item.max_stack_size();
        item.copy_with_count(max)
    }
}
