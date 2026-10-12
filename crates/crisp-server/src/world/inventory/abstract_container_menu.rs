use crisp_data::packets::play::clientbound;
use crisp_data::{registry, registry_id};

use crisp_core::BlockPos;

use super::beacon_menu::PAYMENT_SLOT;
use super::enchantment_menu::EnchantmentMenu;
use super::slot::{Slot, SlotContainer, SlotKind, brewing_stand_fuel_may_place_item, is_fuel};
use crate::network::hashed_stack::HashedStack;
use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::item::crafting::potion_ingredient::PotionIngredient;
use crate::world::item::crafting::{self, CookingRecipeType};
use crate::world::item::{ItemStack, write_slot};

pub const PICKUP: i32 = 0;
pub const QUICK_MOVE: i32 = 1;
pub const SWAP: i32 = 2;
pub const CLONE: i32 = 3;
pub const THROW: i32 = 4;
pub const QUICK_CRAFT: i32 = 5;
pub const PICKUP_ALL: i32 = 6;
pub const SLOT_CLICKED_OUTSIDE: i32 = -999;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind {
    Chest { rows: usize },
    ShulkerBox,
    Furnace(CookingRecipeType),
    Hopper,
    Dispenser,
    BrewingStand,
    Lectern,
    Beacon,
    Enchantment,
}

impl MenuKind {
    pub fn menu_type(self) -> &'static str {
        match self {
            MenuKind::Chest { rows: 3 } => "minecraft:generic_9x3",
            MenuKind::Chest { rows: 6 } => "minecraft:generic_9x6",
            MenuKind::Chest { .. } => "minecraft:generic_9x1",
            MenuKind::ShulkerBox => "minecraft:shulker_box",
            MenuKind::Furnace(CookingRecipeType::Smelting) => "minecraft:furnace",
            MenuKind::Furnace(CookingRecipeType::Blasting) => "minecraft:blast_furnace",
            MenuKind::Furnace(CookingRecipeType::Smoking) => "minecraft:smoker",
            MenuKind::Furnace(CookingRecipeType::CampfireCooking) => unreachable!("campfires have no menu"),
            MenuKind::Hopper => "minecraft:hopper",
            MenuKind::Dispenser => "minecraft:generic_3x3",
            MenuKind::BrewingStand => "minecraft:brewing_stand",
            MenuKind::Lectern => "minecraft:lectern",
            MenuKind::Beacon => "minecraft:beacon",
            MenuKind::Enchantment => "minecraft:enchantment",
        }
    }
}

#[derive(Clone, Debug)]
pub enum RemoteSlot {
    Placeholder,
    Synchronized { remote_stack: Option<ItemStack>, remote_hash: Option<HashedStack> },
}

impl RemoteSlot {
    fn synchronized() -> RemoteSlot {
        RemoteSlot::Synchronized { remote_stack: None, remote_hash: None }
    }

    fn force(&mut self, outgoing: &ItemStack) {
        if let RemoteSlot::Synchronized { remote_stack, remote_hash } = self {
            *remote_stack = Some(outgoing.copy());
            *remote_hash = None;
        }
    }

    fn receive(&mut self, incoming: HashedStack) {
        if let RemoteSlot::Synchronized { remote_stack, remote_hash } = self {
            *remote_stack = None;
            *remote_hash = Some(incoming);
        }
    }

    fn matches(&mut self, local: &ItemStack) -> bool {
        match self {
            RemoteSlot::Placeholder => true,
            RemoteSlot::Synchronized { remote_stack, remote_hash } => {
                if let Some(remote) = remote_stack {
                    ItemStack::matches(remote, local)
                } else if remote_hash.as_ref().is_some_and(|h| h.matches(Some(local))) {
                    *remote_stack = Some(local.copy());
                    true
                } else {
                    false
                }
            }
        }
    }
}

pub struct AbstractContainerMenu {
    pub container_id: i32,
    pub kind: MenuKind,
    pub container: ContainerRef,
    pub access: Option<BlockPos>,
    pub enchantment: Option<EnchantmentMenu>,
    pub slots: Vec<Slot>,
    last_slots: Vec<ItemStack>,
    remote_slots: Vec<RemoteSlot>,
    data_prev: Vec<i32>,
    remote_data_slots: Vec<i32>,
    remote_carried: RemoteSlot,
    state_id: i32,
    quickcraft_type: i32,
    quickcraft_status: i32,
    quickcraft_slots: Vec<usize>,
    suppress_remote_updates: bool,
    has_synchronizer: bool,
}

impl AbstractContainerMenu {
    pub fn new(container_id: i32, kind: MenuKind, container: ContainerRef) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu {
            container_id,
            kind,
            container,
            access: None,
            enchantment: None,
            slots: Vec::new(),
            last_slots: Vec::new(),
            remote_slots: Vec::new(),
            data_prev: Vec::new(),
            remote_data_slots: Vec::new(),
            remote_carried: RemoteSlot::Placeholder,
            state_id: 0,
            quickcraft_type: -1,
            quickcraft_status: 0,
            quickcraft_slots: Vec::new(),
            suppress_remote_updates: false,
            has_synchronizer: false,
        };
        let c = SlotContainer::Container(container);
        match kind {
            MenuKind::Chest { rows } => (0..rows * 9).for_each(|i| menu.add_slot(Slot::new(c, i, SlotKind::Normal))),
            MenuKind::ShulkerBox => (0..27).for_each(|i| menu.add_slot(Slot::new(c, i, SlotKind::ShulkerBox))),
            MenuKind::Furnace(_) => {
                menu.add_slot(Slot::new(c, 0, SlotKind::Normal));
                menu.add_slot(Slot::new(c, 1, SlotKind::FurnaceFuel));
                menu.add_slot(Slot::new(c, 2, SlotKind::FurnaceResult { remove_count: 0 }));
            }
            MenuKind::Hopper => (0..5).for_each(|i| menu.add_slot(Slot::new(c, i, SlotKind::Normal))),
            MenuKind::Dispenser => (0..9).for_each(|i| menu.add_slot(Slot::new(c, i, SlotKind::Normal))),
            MenuKind::BrewingStand => {
                (0..3).for_each(|i| menu.add_slot(Slot::new(c, i, SlotKind::BrewingStandPotion)));
                menu.add_slot(Slot::new(c, 3, SlotKind::BrewingStandIngredients));
                menu.add_slot(Slot::new(c, 4, SlotKind::BrewingStandFuel));
            }
            MenuKind::Lectern => {
                menu.add_slot(Slot::new(c, 0, SlotKind::Normal));
                menu.data_prev = vec![0; 1];
                menu.remote_data_slots = vec![0; 1];
                return menu;
            }
            MenuKind::Beacon => {
                menu.add_slot(Slot::new(c, PAYMENT_SLOT, SlotKind::BeaconPayment));
                menu.data_prev = vec![0; 3];
                menu.remote_data_slots = vec![0; 3];
            }
            MenuKind::Enchantment => {
                menu.add_slot(Slot::new(c, 0, SlotKind::EnchantmentItem));
                menu.add_slot(Slot::new(c, 1, SlotKind::EnchantmentLapis));
                menu.data_prev = vec![0; 10];
                menu.remote_data_slots = vec![0; 10];
            }
        }
        for i in 9..36 {
            menu.add_slot(Slot::new(SlotContainer::Inventory, i, SlotKind::Normal));
        }
        for i in 0..9 {
            menu.add_slot(Slot::new(SlotContainer::Inventory, i, SlotKind::Normal));
        }
        if let MenuKind::Furnace(_) | MenuKind::BrewingStand = kind {
            menu.data_prev = vec![0; 4];
            menu.remote_data_slots = vec![0; 4];
        }
        menu
    }

    fn add_slot(&mut self, mut slot: Slot) {
        slot.index = self.slots.len();
        self.slots.push(slot);
        self.last_slots.push(ItemStack::empty());
        self.remote_slots.push(if self.has_synchronizer { RemoteSlot::synchronized() } else { RemoteSlot::Placeholder });
    }

    pub fn chest_container(&self) -> Option<ContainerRef> {
        matches!(self.kind, MenuKind::Chest { .. }).then_some(self.container)
    }

    pub fn state_id(&self) -> i32 {
        self.state_id
    }

    fn increment_state_id(&mut self) -> i32 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }

    pub fn is_valid_slot_index(&self, slot: i32) -> bool {
        slot == -1 || slot == SLOT_CLICKED_OUTSIDE || slot < self.slots.len() as i32
    }

    pub fn set_remote_slot_unsafe(&mut self, slot: i32, stack: HashedStack) {
        if slot >= 0 && (slot as usize) < self.remote_slots.len() {
            self.remote_slots[slot as usize].receive(stack);
        }
    }

    pub fn set_remote_carried(&mut self, stack: HashedStack) {
        self.remote_carried.receive(stack);
    }

    pub fn set_suppress_remote_updates(&mut self, suppress: bool) {
        self.suppress_remote_updates = suppress;
    }

    fn reset_quick_craft(&mut self) {
        self.quickcraft_status = 0;
        self.quickcraft_slots.clear();
    }

    fn data_count(&self) -> usize {
        self.data_prev.len()
    }
}

fn quickcraft_type(mask: i32) -> i32 {
    mask >> 2 & 3
}

fn quickcraft_header(mask: i32) -> i32 {
    mask & 3
}

fn quick_craft_place_count(slots: usize, ty: i32, stack: &ItemStack) -> i32 {
    match ty {
        0 => (stack.count as f32 / slots as f32).floor() as i32,
        1 => 1,
        2 => stack.max_stack_size(),
        _ => stack.count,
    }
}

impl ServerLevel {
    fn carried(&self, player: EntityId) -> ItemStack {
        self.player(player).carried.clone().unwrap_or_else(ItemStack::empty)
    }

    fn set_carried(&mut self, player: EntityId, stack: ItemStack) {
        self.player_mut(player).carried = (!stack.is_empty()).then_some(stack);
    }

    fn is_valid_quickcraft_type(&self, ty: i32, player: EntityId) -> bool {
        ty == 0 || ty == 1 || ty == 2 && self.player(player).abilities.instabuild
    }

    fn can_item_quick_replace(&mut self, player: EntityId, slot: &Slot, stack: &ItemStack, ignore_size: bool) -> bool {
        let item = self.slot_get(player, slot);
        if !item.is_empty() && ItemStack::is_same_item_same_components(stack, &item) {
            item.count + if ignore_size { 0 } else { stack.count } <= stack.max_stack_size()
        } else {
            item.is_empty()
        }
    }

    pub(super) fn menu_data(&mut self, menu: &AbstractContainerMenu, id: usize) -> i32 {
        if menu.kind == MenuKind::Beacon {
            return menu.access.map_or(0, |pos| self.beacon_data_get(pos, id));
        }
        if let Some(enchantment) = &menu.enchantment {
            return enchantment.get(id);
        }
        if let ContainerRef::Lectern { pos, .. } = menu.container {
            return if id == crate::world::level::block::entity::lectern_block_entity::DATA_PAGE { self.lectern(pos).map_or(0, |lectern| lectern.get_page()) } else { 0 };
        }
        let Some(pos) = menu.container.single_block() else { return 0 };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(crate::world::level::block::entity::BlockEntityKind::Furnace(furnace)) => furnace.data(id),
            Some(crate::world::level::block::entity::BlockEntityKind::BrewingStand(brewing_stand)) => brewing_stand.data(id),
            _ => 0,
        }
    }

    pub fn menu_clicked(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, slot_index: i32, button: i32, input: i32) -> Result<(), ()> {
        let slot_at = |menu: &AbstractContainerMenu, i: i32| -> Result<usize, ()> { if i >= 0 && (i as usize) < menu.slots.len() { Ok(i as usize) } else { Err(()) } };
        if input == QUICK_CRAFT {
            let expected_status = menu.quickcraft_status;
            menu.quickcraft_status = quickcraft_header(button);
            if (expected_status != 1 || menu.quickcraft_status != 2) && expected_status != menu.quickcraft_status {
                menu.reset_quick_craft();
            } else if self.carried(player).is_empty() {
                menu.reset_quick_craft();
            } else if menu.quickcraft_status == 0 {
                menu.quickcraft_type = quickcraft_type(button);
                if self.is_valid_quickcraft_type(menu.quickcraft_type, player) {
                    menu.quickcraft_status = 1;
                    menu.quickcraft_slots.clear();
                } else {
                    menu.reset_quick_craft();
                }
            } else if menu.quickcraft_status == 1 {
                let i = slot_at(menu, slot_index)?;
                let slot = menu.slots[i].clone();
                let carried = self.carried(player);
                if self.can_item_quick_replace(player, &slot, &carried, true)
                    && self.slot_may_place(&slot, &carried)
                    && (menu.quickcraft_type == 2 || carried.count > menu.quickcraft_slots.len() as i32)
                    && !menu.quickcraft_slots.contains(&i)
                {
                    menu.quickcraft_slots.push(i);
                }
            } else if menu.quickcraft_status == 2 {
                if !menu.quickcraft_slots.is_empty() {
                    if menu.quickcraft_slots.len() == 1 {
                        let slot = menu.quickcraft_slots[0] as i32;
                        menu.reset_quick_craft();
                        return self.menu_clicked(menu, player, slot, menu.quickcraft_type, PICKUP);
                    }
                    let source = self.carried(player).copy();
                    if source.is_empty() {
                        menu.reset_quick_craft();
                        return Ok(());
                    }
                    let mut remaining = self.carried(player).count;
                    let size = menu.quickcraft_slots.len();
                    for &i in &menu.quickcraft_slots.clone() {
                        let slot = menu.slots[i].clone();
                        let carried = self.carried(player);
                        if self.can_item_quick_replace(player, &slot, &carried, true)
                            && self.slot_may_place(&slot, &carried)
                            && (menu.quickcraft_type == 2 || carried.count >= size as i32)
                        {
                            let current = self.slot_get(player, &slot);
                            let carry = if current.is_empty() { 0 } else { current.count };
                            let max_size = source.max_stack_size().min(self.slot_max_stack_size(&slot, &source));
                            let new_count = (quick_craft_place_count(size, menu.quickcraft_type, &source) + carry).min(max_size);
                            remaining -= new_count - carry;
                            self.slot_set_by_player(player, &slot, source.copy_with_count(new_count));
                        }
                    }
                    let mut source = source;
                    source.count = remaining;
                    self.set_carried(player, source);
                }
                menu.reset_quick_craft();
            } else {
                menu.reset_quick_craft();
            }
        } else if menu.quickcraft_status != 0 {
            menu.reset_quick_craft();
        } else if (input == PICKUP || input == QUICK_MOVE) && (button == 0 || button == 1) {
            let primary = button == 0;
            if slot_index == SLOT_CLICKED_OUTSIDE {
                let carried = self.carried(player);
                if !carried.is_empty() {
                    if primary {
                        self.player_drop(player, carried, true, Prediction::Predicted);
                        self.set_carried(player, ItemStack::empty());
                    } else {
                        let mut carried = carried;
                        let one = carried.split(1);
                        self.set_carried(player, carried);
                        self.player_drop(player, one, true, Prediction::Predicted);
                    }
                }
            } else if input == QUICK_MOVE {
                if slot_index < 0 {
                    return Ok(());
                }
                let i = slot_at(menu, slot_index)?;
                if !self.slot_may_pickup(player, &menu.slots[i]) {
                    return Ok(());
                }
                let mut clicked = self.quick_move_stack(menu, player, i);
                while !clicked.is_empty() && ItemStack::is_same_item(&self.slot_get(player, &menu.slots[i].clone()), &clicked) {
                    clicked = self.quick_move_stack(menu, player, i);
                }
            } else {
                if slot_index < 0 {
                    return Ok(());
                }
                let i = slot_at(menu, slot_index)?;
                let mut slot = menu.slots[i].clone();
                let clicked = self.slot_get(player, &slot);
                let mut carried = self.carried(player);
                if clicked.is_empty() {
                    if !carried.is_empty() {
                        let amount = if primary { carried.count } else { 1 };
                        let rest = self.slot_safe_insert(player, &slot, carried, amount);
                        self.set_carried(player, rest);
                    }
                } else if self.slot_may_pickup(player, &slot) {
                    if carried.is_empty() {
                        let amount = if primary { clicked.count } else { (clicked.count + 1) / 2 };
                        if let Some(taken) = self.slot_try_remove(player, &mut slot, amount, i32::MAX) {
                            self.set_carried(player, taken.clone());
                            self.slot_on_take(player, &mut slot, &taken);
                        }
                    } else if self.slot_may_place(&slot, &carried) {
                        if ItemStack::is_same_item_same_components(&clicked, &carried) {
                            let amount = if primary { carried.count } else { 1 };
                            let rest = self.slot_safe_insert(player, &slot, carried, amount);
                            self.set_carried(player, rest);
                        } else if carried.count <= self.slot_max_stack_size(&slot, &carried) {
                            self.set_carried(player, clicked);
                            self.slot_set_by_player(player, &slot, carried);
                        }
                    } else if ItemStack::is_same_item_same_components(&clicked, &carried) {
                        if let Some(taken) = self.slot_try_remove(player, &mut slot, clicked.count, carried.max_stack_size() - carried.count) {
                            carried.grow(taken.count);
                            self.set_carried(player, carried);
                            self.slot_on_take(player, &mut slot, &taken);
                        }
                    }
                }
                self.slot_set_changed(&slot);
                menu.slots[i] = slot;
            }
        } else if input == SWAP && (0..9).contains(&button) || input == SWAP && button == 40 {
            let inventory_slot = crate::world::item::inventory_slot(button as usize);
            let source = self.player(player).inventory.slots[inventory_slot].clone().unwrap_or_else(ItemStack::empty);
            let i = slot_at(menu, slot_index)?;
            let mut target = menu.slots[i].clone();
            let target_stack = self.slot_get(player, &target);
            if !source.is_empty() || !target_stack.is_empty() {
                let set_inventory = |level: &mut ServerLevel, stack: ItemStack| {
                    level.player_mut(player).inventory.slots[inventory_slot] = (!stack.is_empty()).then_some(stack);
                };
                if source.is_empty() {
                    if self.slot_may_pickup(player, &target) {
                        set_inventory(self, target_stack.clone());
                        self.slot_set_by_player(player, &target, ItemStack::empty());
                        self.slot_on_take(player, &mut target, &target_stack);
                    }
                } else if target_stack.is_empty() {
                    if self.slot_may_place(&target, &source) {
                        let max = self.slot_max_stack_size(&target, &source);
                        if source.count > max {
                            let mut source = source;
                            let piece = source.split(max);
                            set_inventory(self, source);
                            self.slot_set_by_player(player, &target, piece);
                        } else {
                            set_inventory(self, ItemStack::empty());
                            self.slot_set_by_player(player, &target, source);
                        }
                    }
                } else if self.slot_may_pickup(player, &target) && self.slot_may_place(&target, &source) {
                    let max = self.slot_max_stack_size(&target, &source);
                    if source.count > max {
                        let mut source = source;
                        let piece = source.split(max);
                        set_inventory(self, source);
                        self.slot_set_by_player(player, &target, piece);
                        self.slot_on_take(player, &mut target, &target_stack);
                        let mut rest = target_stack;
                        let infinite_materials = self.player(player).abilities.instabuild;
                        let p = self.player_mut(player);
                        let (_, changed) = p.inventory.add_stack(&mut rest, infinite_materials);
                        p.dirty_slots.extend(changed);
                        self.drop_from_player_if_any(player, rest);
                    } else {
                        set_inventory(self, target_stack.clone());
                        self.slot_set_by_player(player, &target, source);
                        self.slot_on_take(player, &mut target, &target_stack);
                    }
                }
            }
            menu.slots[i] = target;
        } else if input == CLONE && self.player(player).abilities.instabuild && self.carried(player).is_empty() && slot_index >= 0 {
            let i = slot_at(menu, slot_index)?;
            let slot = menu.slots[i].clone();
            if self.slot_has_item(player, &slot) {
                let clone = self.slot_safe_clone(player, &slot);
                self.set_carried(player, clone);
            }
        } else if input == THROW && self.carried(player).is_empty() && slot_index >= 0 {
            let i = slot_at(menu, slot_index)?;
            let mut slot = menu.slots[i].clone();
            let amount = if button == 0 { 1 } else { self.slot_get(player, &slot).count };
            let mut stack = self.slot_safe_take(player, &mut slot, amount, i32::MAX);
            self.drop_from_player_if_any(player, stack.clone());
            if button == 1 {
                while !stack.is_empty() && ItemStack::is_same_item(&self.slot_get(player, &slot), &stack) {
                    stack = self.slot_safe_take(player, &mut slot, amount, i32::MAX);
                    self.drop_from_player_if_any(player, stack.clone());
                }
            }
            menu.slots[i] = slot;
        } else if input == PICKUP_ALL && slot_index >= 0 {
            let i = slot_at(menu, slot_index)?;
            let slot = menu.slots[i].clone();
            let mut carried = self.carried(player);
            if !carried.is_empty() && (!self.slot_has_item(player, &slot) || !self.slot_may_pickup(player, &slot)) {
                let len = menu.slots.len() as i32;
                let (start, step) = if button == 0 { (0, 1) } else { (len - 1, -1) };
                for pass in 0..2 {
                    let mut j = start;
                    while j >= 0 && j < len && carried.count < carried.max_stack_size() {
                        let mut target = menu.slots[j as usize].clone();
                        if self.slot_has_item(player, &target) && self.can_item_quick_replace(player, &target, &carried, true) && self.slot_may_pickup(player, &target) {
                            let stack = self.slot_get(player, &target);
                            if pass != 0 || stack.count != stack.max_stack_size() {
                                let removed = self.slot_safe_take(player, &mut target, stack.count, carried.max_stack_size() - carried.count);
                                carried.grow(removed.count);
                                self.set_carried(player, carried.clone());
                            }
                        }
                        menu.slots[j as usize] = target;
                        j += step;
                    }
                }
            }
        }
        Ok(())
    }

    fn drop_from_player_if_any(&mut self, player: EntityId, stack: ItemStack) {
        self.player_drop(player, stack, true, Prediction::Predicted);
    }

    fn move_item_stack_to(&mut self, menu: &AbstractContainerMenu, player: EntityId, stack: &mut ItemStack, start: usize, end: usize, backwards: bool) -> bool {
        let mut anything_changed = false;
        let order: Vec<usize> = if backwards { (start..end).rev().collect() } else { (start..end).collect() };
        if stack.is_stackable() {
            for &i in &order {
                if stack.is_empty() {
                    break;
                }
                let slot = &menu.slots[i];
                let mut target = self.slot_get(player, slot);
                if !target.is_empty() && ItemStack::is_same_item_same_components(stack, &target) {
                    let total = target.count + stack.count;
                    let max = self.slot_max_stack_size(slot, &target);
                    if total <= max {
                        stack.count = 0;
                        target.count = total;
                        self.slot_put(player, slot, target);
                        self.slot_set_changed(slot);
                        anything_changed = true;
                    } else if target.count < max {
                        stack.shrink(max - target.count);
                        target.count = max;
                        self.slot_put(player, slot, target);
                        self.slot_set_changed(slot);
                        anything_changed = true;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for &i in &order {
                let slot = &menu.slots[i];
                let target = self.slot_get(player, slot);
                if target.is_empty() && self.slot_may_place(slot, stack) {
                    let max = self.slot_max_stack_size(slot, stack);
                    let piece = stack.split(stack.count.min(max));
                    self.slot_set_by_player(player, slot, piece);
                    self.slot_set_changed(slot);
                    anything_changed = true;
                    break;
                }
            }
        }
        anything_changed
    }

    fn quick_move_stack(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize) -> ItemStack {
        let mut slot = menu.slots[index].clone();
        let mut stack = self.slot_get(player, &slot);
        if stack.is_empty() {
            return ItemStack::empty();
        }
        let clicked = stack.copy();
        let total = menu.slots.len();
        let moved = match menu.kind {
            MenuKind::Chest { rows } => {
                let size = rows * 9;
                if index < size { self.move_item_stack_to(menu, player, &mut stack, size, total, true) } else { self.move_item_stack_to(menu, player, &mut stack, 0, size, false) }
            }
            MenuKind::ShulkerBox | MenuKind::Hopper | MenuKind::Dispenser => {
                let size = match menu.kind {
                    MenuKind::ShulkerBox => 27,
                    MenuKind::Hopper => 5,
                    _ => 9,
                };
                if index < size { self.move_item_stack_to(menu, player, &mut stack, size, total, true) } else { self.move_item_stack_to(menu, player, &mut stack, 0, size, false) }
            }
            MenuKind::Furnace(recipe_type) => {
                if index == 2 {
                    let moved = self.move_item_stack_to(menu, player, &mut stack, 3, 39, true);
                    if moved {
                        self.slot_put(player, &slot, stack.clone());
                        self.slot_on_quick_craft(player, &mut slot, &stack, &clicked);
                    }
                    moved
                } else if index != 1 && index != 0 {
                    if crafting::property_set_contains(recipe_type, stack.get_item()) {
                        self.move_item_stack_to(menu, player, &mut stack, 0, 1, false)
                    } else if is_fuel(&stack) {
                        self.move_item_stack_to(menu, player, &mut stack, 1, 2, false)
                    } else if (3..30).contains(&index) {
                        self.move_item_stack_to(menu, player, &mut stack, 30, 39, false)
                    } else {
                        (30..39).contains(&index) && self.move_item_stack_to(menu, player, &mut stack, 3, 30, false) || !(30..39).contains(&index)
                    }
                } else {
                    self.move_item_stack_to(menu, player, &mut stack, 3, 39, false)
                }
            }
            MenuKind::BrewingStand => {
                if index > 2 && index != 3 && index != 4 {
                    if brewing_stand_fuel_may_place_item(&clicked) {
                        if self.move_item_stack_to(menu, player, &mut stack, 4, 5, false) {
                            self.slot_put(player, &slot, stack);
                            menu.slots[index] = slot;
                            return ItemStack::empty();
                        }
                        !(self.slot_may_place(&menu.slots[3], &stack) && !self.move_item_stack_to(menu, player, &mut stack, 3, 4, false))
                    } else if self.slot_may_place(&menu.slots[3], &stack) {
                        self.move_item_stack_to(menu, player, &mut stack, 3, 4, false)
                    } else if PotionIngredient::is_potion_input(&clicked) {
                        self.move_item_stack_to(menu, player, &mut stack, 0, 3, false)
                    } else if (5..32).contains(&index) {
                        self.move_item_stack_to(menu, player, &mut stack, 32, 41, false)
                    } else if (32..41).contains(&index) {
                        self.move_item_stack_to(menu, player, &mut stack, 5, 32, false)
                    } else {
                        self.move_item_stack_to(menu, player, &mut stack, 5, 41, false)
                    }
                } else {
                    let moved = self.move_item_stack_to(menu, player, &mut stack, 5, 41, true);
                    if moved {
                        self.slot_on_quick_craft(player, &mut slot, &stack, &clicked);
                    }
                    moved
                }
            }
            MenuKind::Lectern => false,
            MenuKind::Beacon => {
                let payment_slot = menu.slots[PAYMENT_SLOT].clone();
                if index == PAYMENT_SLOT {
                    let moved = self.move_item_stack_to(menu, player, &mut stack, 1, 37, true);
                    if moved {
                        self.slot_on_quick_craft(player, &mut slot, &stack, &clicked);
                    }
                    moved
                } else if !self.slot_has_item(player, &payment_slot) && self.slot_may_place(&payment_slot, &stack) && stack.count == 1 {
                    self.move_item_stack_to(menu, player, &mut stack, 0, 1, false)
                } else if (1..28).contains(&index) {
                    self.move_item_stack_to(menu, player, &mut stack, 28, 37, false)
                } else if (28..37).contains(&index) {
                    self.move_item_stack_to(menu, player, &mut stack, 1, 28, false)
                } else {
                    self.move_item_stack_to(menu, player, &mut stack, 1, 37, false)
                }
            }
            MenuKind::Enchantment => {
                if index == 0 || index == 1 {
                    self.move_item_stack_to(menu, player, &mut stack, 2, 38, true)
                } else if stack.is(crisp_data::items::LAPIS_LAZULI) {
                    self.move_item_stack_to(menu, player, &mut stack, 1, 2, true)
                } else {
                    let item_slot = menu.slots[0].clone();
                    if self.slot_has_item(player, &item_slot) || !self.slot_may_place(&item_slot, &stack) {
                        false
                    } else {
                        let single_item = stack.copy_with_count(1);
                        stack.shrink(1);
                        self.slot_set_by_player(player, &item_slot, single_item);
                        true
                    }
                }
            }
        };
        if !moved {
            menu.slots[index] = slot;
            return ItemStack::empty();
        }
        self.slot_put(player, &slot, stack.clone());
        if stack.is_empty() {
            self.slot_set_by_player(player, &slot, ItemStack::empty());
        } else {
            self.slot_set_changed(&slot);
        }
        if matches!(menu.kind, MenuKind::Furnace(_) | MenuKind::Dispenser | MenuKind::BrewingStand | MenuKind::Beacon | MenuKind::Enchantment) {
            if stack.count == clicked.count {
                menu.slots[index] = slot;
                return ItemStack::empty();
            }
            self.slot_on_take(player, &mut slot, &stack);
        }
        menu.slots[index] = slot;
        clicked
    }

    pub fn menu_broadcast_changes(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        for i in 0..menu.slots.len() {
            let current = self.slot_get(player, &menu.slots[i]);
            if !ItemStack::matches(&menu.last_slots[i], &current) {
                menu.last_slots[i] = current.copy();
            }
            if !menu.suppress_remote_updates && !menu.remote_slots[i].matches(&current) {
                menu.remote_slots[i].force(&current);
                if menu.has_synchronizer {
                    let state_id = menu.increment_state_id();
                    let container_id = menu.container_id;
                    self.player(player).connection.send(clientbound::CONTAINER_SET_SLOT, |w| {
                        w.varint(container_id);
                        w.varint(state_id);
                        w.i16(i as i16);
                        write_slot(w, Some(&current));
                    });
                }
            }
        }
        if !menu.suppress_remote_updates {
            let carried = self.carried(player);
            if !menu.remote_carried.matches(&carried) {
                menu.remote_carried.force(&carried);
                if menu.has_synchronizer {
                    self.player(player).connection.send(clientbound::SET_CURSOR_ITEM, |w| write_slot(w, Some(&carried)));
                }
            }
        }
        for i in 0..menu.data_count() {
            let current = self.menu_data(menu, i);
            menu.data_prev[i] = current;
            if !menu.suppress_remote_updates && menu.remote_data_slots[i] != current {
                menu.remote_data_slots[i] = current;
                if menu.has_synchronizer {
                    self.send_container_data(player, menu.container_id, i, current);
                }
            }
        }
    }

    pub fn menu_broadcast_full_state(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        for i in 0..menu.slots.len() {
            let current = self.slot_get(player, &menu.slots[i]);
            if !ItemStack::matches(&menu.last_slots[i], &current) {
                menu.last_slots[i] = current.copy();
            }
        }
        for i in 0..menu.data_count() {
            menu.data_prev[i] = self.menu_data(menu, i);
        }
        self.menu_send_all_data_to_remote(menu, player);
    }

    pub fn menu_set_synchronizer(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        menu.has_synchronizer = true;
        menu.remote_carried = RemoteSlot::synchronized();
        menu.remote_slots.iter_mut().for_each(|r| *r = RemoteSlot::synchronized());
        self.menu_send_all_data_to_remote(menu, player);
    }

    pub fn menu_send_all_data_to_remote(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let items: Vec<ItemStack> = (0..menu.slots.len()).map(|i| self.slot_get(player, &menu.slots[i])).collect();
        for (i, item) in items.iter().enumerate() {
            menu.remote_slots[i].force(item);
        }
        let carried = self.carried(player);
        menu.remote_carried.force(&carried);
        for i in 0..menu.data_count() {
            menu.remote_data_slots[i] = self.menu_data(menu, i);
        }
        if menu.has_synchronizer {
            let state_id = menu.increment_state_id();
            let container_id = menu.container_id;
            self.player(player).connection.send(clientbound::CONTAINER_SET_CONTENT, |w| {
                w.varint(container_id);
                w.varint(state_id);
                w.varint(items.len() as i32);
                items.iter().for_each(|s| write_slot(w, Some(s)));
                write_slot(w, Some(&carried));
            });
            for i in 0..menu.data_count() {
                self.send_container_data(player, container_id, i, menu.remote_data_slots[i]);
            }
        }
    }

    fn send_container_data(&self, player: EntityId, container_id: i32, id: usize, value: i32) {
        self.player(player).connection.send(clientbound::CONTAINER_SET_DATA, |w| {
            w.varint(container_id);
            w.i16(id as i16);
            w.i16(value as i16);
        });
    }

    pub fn menu_still_valid(&mut self, menu: &AbstractContainerMenu, player: EntityId) -> bool {
        if menu.kind == MenuKind::Beacon {
            return self.still_valid_access(menu.access, player, crisp_data::blocks::BEACON);
        }
        if menu.kind == MenuKind::Enchantment {
            return self.still_valid_access(menu.access, player, crisp_data::blocks::ENCHANTING_TABLE);
        }
        self.container_still_valid(menu.container, player)
    }

    fn still_valid_access(&mut self, access: Option<BlockPos>, player: EntityId, block: crisp_data::Block) -> bool {
        let Some(pos) = access else { return true };
        crate::world::level::BlockGetter::block_state(self, pos).is_block(block) && self.is_within_block_interaction_range(player, pos, 4.0)
    }

    pub fn menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let carried = self.carried(player);
        if !carried.is_empty() {
            self.drop_or_place_in_inventory(player, carried);
            self.set_carried(player, ItemStack::empty());
        }
        self.container_stop_open(menu.container, player);
        match menu.kind {
            MenuKind::Beacon => self.beacon_menu_removed(menu, player),
            MenuKind::Enchantment => self.enchantment_menu_removed(menu, player),
            _ => {}
        }
    }

    fn drop_or_place_in_inventory(&mut self, player: EntityId, carried: ItemStack) {
        if self.player(player).disconnected {
            self.player_drop(player, carried, false, Prediction::ServerOnly);
        } else {
            self.place_item_back_in_inventory(player, carried);
        }
    }

    pub fn clear_container(&mut self, player: EntityId, container: ContainerRef) {
        for i in 0..self.container_size(container) {
            let stack = self.remove_item_no_update(container, i);
            self.drop_or_place_in_inventory(player, stack);
        }
    }

    pub fn menu_container_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        if !std::mem::take(&mut self.player_mut(player).menu_container.changed) {
            return;
        }
        if menu.kind == MenuKind::Enchantment {
            self.enchantment_menu_slots_changed(menu, player);
        }
    }

    pub fn place_item_back_in_inventory(&mut self, player: EntityId, mut stack: ItemStack) {
        while !stack.is_empty() {
            let inventory = &self.player(player).inventory;
            let Some(slot) = inventory.slot_with_remaining_space(&stack).or_else(|| inventory.free_slot()) else {
                self.player_drop(player, stack, false, Prediction::ServerOnly);
                break;
            };
            let p = self.player_mut(player);
            let in_slot = p.inventory.slots[slot].get_or_insert_with(ItemStack::empty);
            let room = stack.max_stack_size() - in_slot.count;
            let piece = stack.split(room);
            if in_slot.is_empty() {
                *in_slot = piece;
            } else {
                in_slot.grow(piece.count);
            }
            let item = in_slot.clone();
            if let Some(index) = crate::world::item::inventory_index(slot) {
                p.connection.send(clientbound::SET_PLAYER_INVENTORY, |w| {
                    w.varint(index as i32);
                    write_slot(w, Some(&item));
                });
            }
        }
    }
}

pub fn menu_type_id(kind: MenuKind) -> i32 {
    registry_id(registry::MENU, kind.menu_type())
}
