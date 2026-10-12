pub mod alchemy;
pub mod banner_item;
pub mod block_item;
pub mod bucket_item;
pub mod bone_meal_item;
pub mod component;
pub mod context;
pub mod crafting;
pub mod dye_color;
pub mod dye_item;
pub mod empty_map_item;
pub mod glow_ink_sac_item;
pub mod ink_sac_item;
pub mod sign_applicator;
pub mod enchantment;
pub mod ender_eye_item;
pub mod flint_and_steel_item;
mod item_stack;
pub mod honeycomb_item;
pub mod item_utils;
pub mod jukebox_playable;
pub mod jukebox_song;
pub mod jukebox_song_player;
pub mod map_item;
pub mod name_tag_item;
pub mod scaffolding_block_item;
pub mod slot;
pub mod spawn_egg_item;

use crisp_data::Item;

use crate::world::entity::player::stacked_item_contents::StackedItemContents;

pub use item_stack::{ItemStack, read_slot, validate_encodable, write_slot};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::block::BlockHitResult;
use crate::world::{InteractionHand, InteractionResult};
use context::BlockPlaceContext;

static BLOCK_ITEMS: std::sync::LazyLock<Vec<Option<Box<dyn block_item::BlockItemBehaviour>>>> =
    std::sync::LazyLock::new(|| (0..Item::count()).map(|i| block_item::for_item(Item(i as u16))).collect());

pub fn interact_living_entity(level: &mut ServerLevel, item_stack: &mut ItemStack, player: EntityId, target: EntityId, _hand: InteractionHand) -> InteractionResult {
    if item_stack.is(crisp_data::items::NAME_TAG) {
        return name_tag_item::interact_living_entity(level, item_stack, player, target);
    }
    InteractionResult::Pass
}

pub fn block_item_behaviour(item: Item) -> Option<&'static dyn block_item::BlockItemBehaviour> {
    BLOCK_ITEMS[item.0 as usize].as_deref()
}

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, stack: ItemStack) -> InteractionResult {
    match stack.item.info().classes.first() {
        Some(&"BoneMealItem") => return bone_meal_item::use_on(level, player, hand, hit, stack),
        Some(&"FlintAndSteelItem") => {
            let ctx = BlockPlaceContext::new(level, Some(player), hand, stack, hit);
            return flint_and_steel_item::flint_and_steel_use_on(level, player, hand, &ctx);
        }
        Some(&"FireChargeItem") => {
            let ctx = BlockPlaceContext::new(level, Some(player), hand, stack, hit);
            return flint_and_steel_item::fire_charge_use_on(level, player, hand, &ctx);
        }
        Some(&"SpawnEggItem") => return spawn_egg_item::use_on(level, player, hand, hit, stack),
        Some(&"HoneycombItem") => return honeycomb_item::use_on(level, player, hand, hit, stack),
        Some(&"EnderEyeItem") => return ender_eye_item::use_on(level, player, hand, hit, stack),
        Some(&"MapItem") => {
            if let Some(result) = map_item::use_on(level, &hit, &stack) {
                return result;
            }
        }
        _ => {}
    }
    match &BLOCK_ITEMS[stack.item.0 as usize] {
        Some(item) => {
            let ctx = BlockPlaceContext::new(level, Some(player), hand, stack, hit);
            block_item::place(item.as_ref(), level, ctx)
        }
        None => match stack.get(component::BLOCK_TRANSFORMER).and_then(crate::core::component::block_transformer::BlockTransformer::get) {
            Some(block_transformer) => {
                let ctx = BlockPlaceContext::new(level, Some(player), hand, stack, hit);
                block_transformer.transform_block(level, &ctx)
            }
            None => InteractionResult::Pass,
        },
    }
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, stack: ItemStack) -> (InteractionResult, Option<ItemStack>) {
    if stack.item.is_instance_of("SpawnEggItem") {
        return spawn_egg_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("EnderEyeItem") {
        return ender_eye_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("EmptyMapItem") {
        return empty_map_item::use_item(level, player, hand);
    }
    match bucket_item::BucketItem::for_item(stack.item) {
        Some(bucket) => bucket.use_item(level, player, hand),
        None => match &stack.item.info().consumable {
            Some(consumable) => {
                let instant = consumable.consume_ticks() <= 0;
                let (result, transformed) = level.start_consuming(player, hand, stack.clone(), consumable);
                match (instant, result, transformed) {
                    (true, InteractionResult::Success { .. }, Some(t)) => (result, Some(level.apply_after_use_component_side_effects(player, t, stack))),
                    (_, result, transformed) => (result, transformed),
                }
            }
            None => (InteractionResult::Pass, None),
        },
    }
}

pub fn inventory_tick(level: &mut ServerLevel, owner: EntityId, slot_index: usize, slot: Option<crisp_data::EquipmentSlot>) {
    let Some(item_stack) = level.player(owner).inventory.slots[slot_index].as_ref().filter(|stack| !stack.is_empty()) else { return };
    if item_stack.item.is_instance_of("MapItem") {
        let item_stack = item_stack.clone();
        map_item::inventory_tick(&item_stack, level, owner, slot);
    }
}

pub fn on_crafted_by(item_stack: &mut ItemStack, level: &mut ServerLevel, _player: EntityId) {
    on_crafted_post_process(item_stack, level);
}

pub fn on_crafted_post_process(item_stack: &mut ItemStack, level: &mut ServerLevel) {
    if item_stack.item.is_instance_of("MapItem") {
        map_item::on_crafted_post_process(item_stack, level);
    }
}

pub fn can_fit_inside_container_items(item: Item) -> bool {
    !item.info().block.is_some_and(|block| block.is_instance_of("ShulkerBoxBlock"))
}

pub const HOTBAR_START: usize = 36;
pub const MAIN_START: usize = 9;
pub const OFFHAND: usize = 45;
pub const SLOTS: usize = 46;

pub fn inventory_slot(index: usize) -> usize {
    match index {
        0..=8 => HOTBAR_START + index,
        36..=39 => 44 - index,
        40 => OFFHAND,
        _ => index,
    }
}

pub fn inventory_index(slot: usize) -> Option<usize> {
    match slot {
        36..=44 => Some(slot - HOTBAR_START),
        9..=35 => Some(slot),
        5..=8 => Some(44 - slot),
        OFFHAND => Some(40),
        _ => None,
    }
}

pub fn protocol_slot(storage: i8) -> Option<usize> {
    match storage {
        0..=8 => Some(storage as usize + 36),
        9..=35 => Some(storage as usize),
        100..=103 => Some(108 - storage as usize),
        -106 => Some(OFFHAND),
        _ => None,
    }
}

pub struct Inventory {
    pub slots: [Option<ItemStack>; SLOTS],
    pub selected: usize,
}

impl Inventory {
    pub fn new() -> Inventory {
        Inventory { slots: std::array::from_fn(|_| None), selected: 0 }
    }

    pub fn selected_slot(&self) -> usize {
        HOTBAR_START + self.selected
    }

    pub fn held(&self) -> Option<&ItemStack> {
        self.slots[self.selected_slot()].as_ref()
    }

    fn item(&self, index: usize) -> Option<&ItemStack> {
        self.slots[inventory_slot(index)].as_ref().filter(|s| !s.is_empty())
    }

    pub fn is_hotbar_slot(index: usize) -> bool {
        index < 9
    }

    pub fn is_usable_for_crafting(item_stack: &ItemStack) -> bool {
        !item_stack.is_damaged() && !item_stack.is_enchanted() && !item_stack.has(component::CUSTOM_NAME)
    }

    pub fn find_slot_matching_item(&self, stack: &ItemStack) -> Option<usize> {
        (0..36).find(|&i| self.item(i).is_some_and(|item| ItemStack::is_same_item_same_components(stack, item)))
    }

    fn get_free_slot(&self) -> Option<usize> {
        (0..36).find(|&i| self.item(i).is_none())
    }

    pub fn find_slot_matching_crafting_ingredient(&self, item: Item, existing_item: &ItemStack) -> Option<usize> {
        (0..36).find(|&i| {
            self.item(i).is_some_and(|inventory_item_stack| {
                inventory_item_stack.is(item)
                    && Inventory::is_usable_for_crafting(inventory_item_stack)
                    && (existing_item.is_empty() || ItemStack::is_same_item_same_components(existing_item, inventory_item_stack))
            })
        })
    }

    pub fn get_item(&self, index: usize) -> ItemStack {
        self.item(index).cloned().unwrap_or_else(ItemStack::empty)
    }

    pub fn remove_item(&mut self, index: usize, count: i32) -> ItemStack {
        match &mut self.slots[inventory_slot(index)] {
            Some(item_stack) if !item_stack.is_empty() && count > 0 => item_stack.split(count),
            _ => ItemStack::empty(),
        }
    }

    pub fn remove_item_no_update(&mut self, index: usize) -> ItemStack {
        self.slots[inventory_slot(index)].take().unwrap_or_else(ItemStack::empty)
    }

    pub fn fill_stacked_contents(&self, contents: &mut StackedItemContents) {
        (0..36).filter_map(|i| self.item(i)).for_each(|item_stack| contents.account_simple_stack(item_stack));
    }

    pub fn get_suitable_hotbar_slot(&self) -> usize {
        let hotbar = || (0..9).map(|slot| (self.selected + slot) % 9);
        hotbar()
            .find(|&i| self.item(i).is_none())
            .or_else(|| hotbar().find(|&i| !self.item(i).is_some_and(ItemStack::is_enchanted)))
            .unwrap_or(self.selected)
    }

    pub fn pick_slot(&mut self, index: usize) -> [usize; 2] {
        self.selected = self.get_suitable_hotbar_slot();
        let (held, picked) = (inventory_slot(self.selected), inventory_slot(index));
        self.slots.swap(held, picked);
        [held, picked]
    }

    pub fn add_and_pick_item(&mut self, stack: ItemStack) -> Vec<usize> {
        self.selected = self.get_suitable_hotbar_slot();
        let held = inventory_slot(self.selected);
        let mut changed = vec![held];
        if self.item(self.selected).is_some()
            && let Some(free) = self.get_free_slot()
        {
            let free = inventory_slot(free);
            self.slots[free] = self.slots[held].clone();
            changed.push(free);
        }
        self.slots[held] = Some(stack);
        changed
    }

    fn storage_order() -> impl Iterator<Item = usize> {
        (HOTBAR_START..HOTBAR_START + 9).chain(MAIN_START..HOTBAR_START)
    }

    pub fn add(&mut self, mut stack: ItemStack) -> (Vec<usize>, i32) {
        let mut changed = Vec::new();
        let max = stack.max_stack_size();
        let partial = std::iter::once(self.selected_slot()).chain([OFFHAND]).chain(Self::storage_order());
        for slot in partial.collect::<Vec<_>>() {
            if stack.count == 0 {
                break;
            }
            if let Some(existing) = &mut self.slots[slot] {
                if ItemStack::is_same_item_same_components(existing, &stack) && existing.count < max {
                    let moved = stack.count.min(max - existing.count);
                    existing.count += moved;
                    stack.count -= moved;
                    if !changed.contains(&slot) {
                        changed.push(slot);
                    }
                }
            }
        }
        for slot in Self::storage_order() {
            if stack.count == 0 {
                break;
            }
            if self.slots[slot].is_none() {
                let moved = stack.count.min(max);
                self.slots[slot] = Some(stack.copy_with_count(moved));
                stack.count -= moved;
                changed.push(slot);
            }
        }
        (changed, stack.count)
    }

    pub(crate) fn free_slot(&self) -> Option<usize> {
        Self::storage_order().find(|&slot| self.slots[slot].as_ref().is_none_or(|s| s.is_empty()))
    }

    pub(crate) fn slot_with_remaining_space(&self, stack: &ItemStack) -> Option<usize> {
        let has_space = |slot: usize| {
            self.slots[slot].as_ref().is_some_and(|s| !s.is_empty() && ItemStack::is_same_item_same_components(s, stack) && s.is_stackable() && s.count < s.max_stack_size().min(99))
        };
        std::iter::once(self.selected_slot()).chain([OFFHAND]).chain(Self::storage_order()).find(|&slot| has_space(slot))
    }

    fn add_resource(&mut self, stack: &ItemStack, changed: &mut Vec<usize>) -> i32 {
        let Some(slot) = self.slot_with_remaining_space(stack).or_else(|| self.free_slot()) else { return stack.count };
        if self.slots[slot].as_ref().is_none_or(|s| s.is_empty()) {
            self.slots[slot] = Some(stack.copy_with_count(0));
        }
        let in_slot = self.slots[slot].as_mut().unwrap();
        let to_add = stack.count.min(in_slot.max_stack_size().min(99) - in_slot.count);
        in_slot.count += to_add;
        if !changed.contains(&slot) {
            changed.push(slot);
        }
        stack.count - to_add
    }

    pub fn add_stack(&mut self, stack: &mut ItemStack, infinite_materials: bool) -> (bool, Vec<usize>) {
        let mut changed = Vec::new();
        if stack.is_empty() {
            return (false, changed);
        }
        if stack.is_damaged() {
            return match self.free_slot() {
                Some(slot) => {
                    self.slots[slot] = Some(stack.clone());
                    stack.count = 0;
                    changed.push(slot);
                    (true, changed)
                }
                None if infinite_materials => {
                    stack.count = 0;
                    (true, changed)
                }
                None => (false, changed),
            };
        }
        let mut last;
        loop {
            last = stack.count;
            stack.count = self.add_resource(stack, &mut changed);
            if stack.is_empty() || stack.count >= last {
                break;
            }
        }
        if stack.count == last && infinite_materials {
            stack.count = 0;
            return (true, changed);
        }
        (stack.count < last, changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crisp_data::items;
    use crisp_protocol::Writer;

    #[test]
    fn first_pickup_goes_to_first_hotbar_slot() {
        let dirt = items::DIRT;
        let mut inv = Inventory::new();
        assert_eq!(inv.add(ItemStack::new(dirt, 1)), (vec![36], 0));
        assert_eq!(inv.add(ItemStack::new(dirt, 70)), (vec![36, 37], 0));
        assert_eq!(inv.slots[36], Some(ItemStack::new(dirt, 64)));
        assert_eq!(inv.slots[37], Some(ItemStack::new(dirt, 7)));
    }

    #[test]
    fn stacks_merge_only_with_equal_components() {
        let mut named = ItemStack::new(items::DIRT, 3);
        named.set(component::CUSTOM_NAME, Some(crisp_protocol::nbt::Nbt::String("x".into())));
        let mut inv = Inventory::new();
        assert_eq!(inv.add(ItemStack::new(items::DIRT, 1)), (vec![36], 0));
        assert_eq!(inv.add(named.clone()), (vec![37], 0));
        assert_eq!(inv.add(named.copy_with_count(2)), (vec![37], 0));
        assert_eq!(inv.slots[37], Some(named.copy_with_count(5)));
        let mut plain = ItemStack::new(items::DIRT, 1);
        plain.set(component::MAX_STACK_SIZE, Some(crisp_protocol::nbt::Nbt::Int(64)));
        assert!(plain.components_patch().is_empty());
        assert!(ItemStack::is_same_item_same_components(&plain, &ItemStack::new(items::DIRT, 2)));
    }

    #[test]
    fn slot_encoding_matches_vanilla() {
        let mut w = Writer::new();
        write_slot(&mut w, Some(&ItemStack::new(items::DIRT, 1)));
        assert_eq!(w.as_slice(), &[0x01, 0x37, 0x00, 0x00]);
    }
}
