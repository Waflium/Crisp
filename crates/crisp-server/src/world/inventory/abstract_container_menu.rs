use crisp_core::BlockPos;
use crisp_data::packets::play::clientbound;
use crisp_data::{EquipmentSlot, registry, registry_id};

use super::beacon_menu::PAYMENT_SLOT;
use super::crafter_menu::{INV_SLOT_START, SLOT_COUNT, USE_ROW_SLOT_END};
use super::anvil_menu::{self, AnvilMenu};
use super::grindstone_menu;
use super::item_combiner_menu;
use super::loom_menu::{self, LoomMenu};
use super::smithing_menu::{self, SmithingMenu};
use super::stonecutter_menu::{self, StonecutterMenu};
use super::enchantment_menu::EnchantmentMenu;
use super::result_container::ResultContainer;
use super::slot::{Slot, SlotContainer, SlotKind, brewing_stand_fuel_may_place_item, is_fuel};
use super::transient_crafting_container::TransientCraftingContainer;
use crate::network::hashed_stack::HashedStack;
use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::item::crafting::potion_ingredient::PotionIngredient;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::crafting::recipe_property_set::{BLAST_FURNACE_INPUT, CAMPFIRE_INPUT, FURNACE_INPUT, RecipePropertySetKey, SMOKER_INPUT};
use crate::world::item::crafting::recipe_type::RecipeType;
use crate::world::item::{ItemStack, write_slot};
use crate::world::level::block::entity::crafter_block_entity::NUM_DATA;
use crate::world::simple_container::{SimpleContainer, SimpleContainerKind};

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
    Inventory,
    Crafting,
    Chest { rows: usize },
    ShulkerBox,
    Furnace(RecipeType),
    Hopper,
    Dispenser,
    BrewingStand,
    Lectern,
    Beacon,
    Enchantment,
    Crafter,
    Stonecutter,
    Smithing,
    Anvil,
    Grindstone,
    Loom,
}

impl MenuKind {
    pub fn menu_type(self) -> &'static str {
        match self {
            MenuKind::Inventory => "",
            MenuKind::Crafting => "minecraft:crafting",
            MenuKind::Chest { rows: 3 } => "minecraft:generic_9x3",
            MenuKind::Chest { rows: 6 } => "minecraft:generic_9x6",
            MenuKind::Chest { .. } => "minecraft:generic_9x1",
            MenuKind::ShulkerBox => "minecraft:shulker_box",
            MenuKind::Furnace(RecipeType::Blasting) => "minecraft:blast_furnace",
            MenuKind::Furnace(RecipeType::Smoking) => "minecraft:smoker",
            MenuKind::Furnace(_) => "minecraft:furnace",
            MenuKind::Hopper => "minecraft:hopper",
            MenuKind::Dispenser => "minecraft:generic_3x3",
            MenuKind::BrewingStand => "minecraft:brewing_stand",
            MenuKind::Lectern => "minecraft:lectern",
            MenuKind::Beacon => "minecraft:beacon",
            MenuKind::Enchantment => "minecraft:enchantment",
            MenuKind::Crafter => "minecraft:crafter_3x3",
            MenuKind::Stonecutter => "minecraft:stonecutter",
            MenuKind::Smithing => "minecraft:smithing",
            MenuKind::Anvil => "minecraft:anvil",
            MenuKind::Grindstone => "minecraft:grindstone",
            MenuKind::Loom => "minecraft:loom",
        }
    }
}

#[derive(Clone, Debug)]
pub enum MenuContainer {
    Simple(SimpleContainer),
    Crafting(TransientCraftingContainer),
    Result(ResultContainer),
}

impl MenuContainer {
    pub fn get_container_size(&self) -> usize {
        match self {
            MenuContainer::Simple(container) => container.items.len(),
            MenuContainer::Crafting(container) => container.items.len(),
            MenuContainer::Result(_) => 1,
        }
    }

    pub fn get_item(&self, slot: usize) -> &ItemStack {
        match self {
            MenuContainer::Simple(container) => &container.items[slot],
            MenuContainer::Crafting(container) => &container.items[slot],
            MenuContainer::Result(container) => &container.item,
        }
    }

    pub fn get_item_mut(&mut self, slot: usize) -> &mut ItemStack {
        match self {
            MenuContainer::Simple(container) => &mut container.items[slot],
            MenuContainer::Crafting(container) => &mut container.items[slot],
            MenuContainer::Result(container) => &mut container.item,
        }
    }

    pub fn get_max_stack_size(&self) -> i32 {
        match self {
            MenuContainer::Simple(container) => container.get_max_stack_size(),
            _ => 99,
        }
    }

    pub fn clear_content(&mut self) {
        match self {
            MenuContainer::Simple(container) => container.items.iter_mut().for_each(|item| *item = ItemStack::empty()),
            MenuContainer::Crafting(container) => container.items.iter_mut().for_each(|item| *item = ItemStack::empty()),
            MenuContainer::Result(container) => container.item = ItemStack::empty(),
        }
    }

    pub fn award_used_recipes(&mut self, level: &mut ServerLevel, player: EntityId) {
        if let MenuContainer::Result(container) = self {
            container.award_used_recipes(level, player);
        }
    }

    pub fn crafting(&self) -> &TransientCraftingContainer {
        match self {
            MenuContainer::Crafting(container) => container,
            _ => panic!("not a crafting container"),
        }
    }

    pub fn result(&mut self) -> &mut ResultContainer {
        match self {
            MenuContainer::Result(container) => container,
            _ => panic!("not a result container"),
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

    fn copy_from(&mut self, other: &RemoteSlot) {
        if let (RemoteSlot::Synchronized { remote_stack, remote_hash }, RemoteSlot::Synchronized { remote_stack: other_stack, remote_hash: other_hash }) = (self, other) {
            remote_stack.clone_from(other_stack);
            remote_hash.clone_from(other_hash);
        }
    }
}

pub struct AbstractContainerMenu {
    pub container_id: i32,
    pub kind: MenuKind,
    pub container: Option<ContainerRef>,
    pub access: Option<BlockPos>,
    pub containers: Vec<MenuContainer>,
    pub enchantment: Option<EnchantmentMenu>,
    pub stonecutter: Option<StonecutterMenu>,
    pub smithing: Option<SmithingMenu>,
    pub anvil: Option<AnvilMenu>,
    pub loom: Option<LoomMenu>,
    pub placing_recipe: bool,
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

const ARMOR_SLOTS: [(EquipmentSlot, usize); 4] = [(EquipmentSlot::Head, 39), (EquipmentSlot::Chest, 38), (EquipmentSlot::Legs, 37), (EquipmentSlot::Feet, 36)];

impl AbstractContainerMenu {
    pub fn new(container_id: i32, kind: MenuKind, container: Option<ContainerRef>) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu {
            container_id,
            kind,
            container,
            access: None,
            containers: Vec::new(),
            enchantment: None,
            stonecutter: None,
            smithing: None,
            anvil: None,
            loom: None,
            placing_recipe: false,
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
        let c = container.map_or(SlotContainer::Menu(0), SlotContainer::Container);
        match kind {
            MenuKind::Inventory | MenuKind::Crafting => {
                let size = if kind == MenuKind::Inventory { 2 } else { 3 };
                menu.containers.push(MenuContainer::Crafting(TransientCraftingContainer::new(size, size)));
                menu.containers.push(MenuContainer::Result(ResultContainer::default()));
                menu.add_slot(Slot::new(SlotContainer::Menu(1), 0, SlotKind::Result { remove_count: 0, craft_slots: 0 }));
                for y in 0..size {
                    for x in 0..size {
                        menu.add_slot(Slot::new(SlotContainer::Menu(0), x + y * size, SlotKind::Normal));
                    }
                }
                if kind == MenuKind::Inventory {
                    for (equipment_slot, index) in ARMOR_SLOTS {
                        menu.add_slot(Slot::new(SlotContainer::Inventory, index, SlotKind::Armor(equipment_slot)));
                    }
                }
            }
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
                menu.containers.push(MenuContainer::Simple(SimpleContainer::new(SimpleContainerKind::BeaconPayment, 1)));
                menu.add_slot(Slot::new(SlotContainer::Menu(0), PAYMENT_SLOT, SlotKind::BeaconPayment));
                menu.data_prev = vec![0; 3];
                menu.remote_data_slots = vec![0; 3];
            }
            MenuKind::Enchantment => {
                menu.containers.push(MenuContainer::Simple(SimpleContainer::new(SimpleContainerKind::Plain, 2)));
                menu.add_slot(Slot::new(SlotContainer::Menu(0), 0, SlotKind::EnchantmentItem));
                menu.add_slot(Slot::new(SlotContainer::Menu(0), 1, SlotKind::EnchantmentLapis));
                menu.data_prev = vec![0; 10];
                menu.remote_data_slots = vec![0; 10];
            }
            MenuKind::Crafter => (0..SLOT_COUNT).for_each(|i| menu.add_slot(Slot::new(c, i, SlotKind::Crafter))),
            MenuKind::Stonecutter => {
                menu.containers.push(MenuContainer::Simple(SimpleContainer::new(SimpleContainerKind::Plain, 1)));
                menu.containers.push(MenuContainer::Result(ResultContainer::default()));
                menu.add_slot(Slot::new(SlotContainer::Menu(stonecutter_menu::CONTAINER), 0, SlotKind::Normal));
                menu.add_slot(Slot::new(SlotContainer::Menu(stonecutter_menu::RESULT_CONTAINER), 1, SlotKind::StonecutterResult));
                menu.data_prev = vec![0; 1];
                menu.remote_data_slots = vec![0; 1];
            }
            MenuKind::Smithing | MenuKind::Anvil => {
                let result_slot = if kind == MenuKind::Smithing { smithing_menu::RESULT_SLOT } else { anvil_menu::RESULT_SLOT };
                menu.containers.push(MenuContainer::Simple(SimpleContainer::new(SimpleContainerKind::Plain, result_slot)));
                menu.containers.push(MenuContainer::Result(ResultContainer::default()));
                (0..result_slot).for_each(|i| menu.add_slot(Slot::new(SlotContainer::Menu(item_combiner_menu::INPUT_SLOTS), i, SlotKind::ItemCombinerInput)));
                menu.add_slot(Slot::new(SlotContainer::Menu(item_combiner_menu::RESULT_SLOTS), result_slot, SlotKind::ItemCombinerResult));
                menu.data_prev = vec![0; 1];
                menu.remote_data_slots = vec![0; 1];
            }
            MenuKind::Grindstone => {
                menu.containers.push(MenuContainer::Simple(SimpleContainer::new(SimpleContainerKind::Plain, 2)));
                menu.containers.push(MenuContainer::Result(ResultContainer::default()));
                menu.add_slot(Slot::new(SlotContainer::Menu(grindstone_menu::REPAIR_SLOTS), 0, SlotKind::GrindstoneInput));
                menu.add_slot(Slot::new(SlotContainer::Menu(grindstone_menu::REPAIR_SLOTS), 1, SlotKind::GrindstoneInput));
                menu.add_slot(Slot::new(SlotContainer::Menu(grindstone_menu::RESULT_SLOTS), 2, SlotKind::GrindstoneResult));
            }
            MenuKind::Loom => {
                menu.containers.push(MenuContainer::Simple(SimpleContainer::new(SimpleContainerKind::Plain, 3)));
                menu.containers.push(MenuContainer::Simple(SimpleContainer::new(SimpleContainerKind::Plain, 1)));
                menu.add_slot(Slot::new(SlotContainer::Menu(loom_menu::INPUT_CONTAINER), 0, SlotKind::LoomBanner));
                menu.add_slot(Slot::new(SlotContainer::Menu(loom_menu::INPUT_CONTAINER), 1, SlotKind::LoomDye));
                menu.add_slot(Slot::new(SlotContainer::Menu(loom_menu::INPUT_CONTAINER), 2, SlotKind::LoomPattern));
                menu.add_slot(Slot::new(SlotContainer::Menu(loom_menu::OUTPUT_CONTAINER), 0, SlotKind::LoomResult));
            }
        }
        for i in 9..36 {
            menu.add_slot(Slot::new(SlotContainer::Inventory, i, SlotKind::Normal));
        }
        for i in 0..9 {
            menu.add_slot(Slot::new(SlotContainer::Inventory, i, SlotKind::Normal));
        }
        if kind == MenuKind::Inventory {
            menu.add_slot(Slot::new(SlotContainer::Inventory, 40, SlotKind::Offhand));
        }
        if let MenuKind::Furnace(_) | MenuKind::BrewingStand = kind {
            menu.data_prev = vec![0; 4];
            menu.remote_data_slots = vec![0; 4];
        }
        if kind == MenuKind::Loom {
            menu.data_prev = vec![0; 1];
            menu.remote_data_slots = vec![0; 1];
        }
        if kind == MenuKind::Crafter {
            menu.containers.push(MenuContainer::Result(ResultContainer::default()));
            menu.add_slot(Slot::new(SlotContainer::Menu(0), 0, SlotKind::NonInteractiveResult));
            menu.data_prev = vec![0; NUM_DATA];
            menu.remote_data_slots = vec![0; NUM_DATA];
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
        if let MenuKind::Chest { .. } = self.kind { self.container } else { None }
    }

    pub fn state_id(&self) -> i32 {
        self.state_id
    }

    pub fn increment_state_id(&mut self) -> i32 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }

    pub fn is_valid_slot_index(&self, slot: i32) -> bool {
        slot == -1 || slot == SLOT_CLICKED_OUTSIDE || slot < self.slots.len() as i32
    }

    pub fn set_remote_slot(&mut self, slot: usize, stack: &ItemStack) {
        self.remote_slots[slot].force(stack);
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

    pub fn transfer_state(&mut self, other_container: &AbstractContainerMenu) {
        for slot_index in 0..self.slots.len() {
            let slot = &self.slots[slot_index];
            if slot.container != SlotContainer::Inventory {
                continue;
            }
            let Some(other_slot_index) = other_container.slots.iter().position(|other| other.container == SlotContainer::Inventory && other.slot == slot.slot) else { continue };
            self.last_slots[slot_index] = other_container.last_slots[other_slot_index].clone();
            self.remote_slots[slot_index].copy_from(&other_container.remote_slots[other_slot_index]);
        }
    }
}

fn furnace_allowed_inputs(recipe_type: RecipeType) -> RecipePropertySetKey {
    match recipe_type {
        RecipeType::Blasting => BLAST_FURNACE_INPUT,
        RecipeType::Smoking => SMOKER_INPUT,
        RecipeType::CampfireCooking => CAMPFIRE_INPUT,
        _ => FURNACE_INPUT,
    }
}

fn humanoid_armor_index(slot: EquipmentSlot) -> Option<usize> {
    match slot {
        EquipmentSlot::Feet => Some(0),
        EquipmentSlot::Legs => Some(1),
        EquipmentSlot::Chest => Some(2),
        EquipmentSlot::Head => Some(3),
        _ => None,
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

    fn can_item_quick_replace(&mut self, menu: &AbstractContainerMenu, player: EntityId, index: usize, stack: &ItemStack, ignore_size: bool) -> bool {
        let item = self.slot_get(menu, player, index);
        if !item.is_empty() && ItemStack::is_same_item_same_components(stack, &item) {
            item.count + if ignore_size { 0 } else { stack.count } <= stack.max_stack_size()
        } else {
            item.is_empty()
        }
    }

    fn can_take_item_for_pick_all(menu: &AbstractContainerMenu, index: usize) -> bool {
        match menu.slots[index].kind {
            SlotKind::Result { .. } | SlotKind::StonecutterResult => false,
            SlotKind::ItemCombinerResult => menu.kind != MenuKind::Smithing,
            _ => true,
        }
    }

    pub(super) fn menu_data(&mut self, menu: &AbstractContainerMenu, id: usize) -> i32 {
        if menu.kind == MenuKind::Beacon {
            return menu.access.map_or(0, |pos| self.beacon_data_get(pos, id));
        }
        if let Some(enchantment) = &menu.enchantment {
            return enchantment.get(id);
        }
        if let Some(stonecutter) = &menu.stonecutter {
            return stonecutter.get_selected_recipe_index();
        }
        if let Some(smithing) = &menu.smithing {
            return smithing.get_has_recipe_error();
        }
        if let Some(anvil) = &menu.anvil {
            return anvil.get_cost();
        }
        if let Some(loom) = &menu.loom {
            return loom.get_selected_banner_pattern_index();
        }
        if let Some(ContainerRef::Lectern { pos, .. }) = menu.container {
            return if id == crate::world::level::block::entity::lectern_block_entity::DATA_PAGE { self.lectern(pos).map_or(0, |lectern| lectern.get_page()) } else { 0 };
        }
        let Some(pos) = menu.container.and_then(ContainerRef::single_block) else { return 0 };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(crate::world::level::block::entity::BlockEntityKind::Furnace(furnace)) => furnace.data(id),
            Some(crate::world::level::block::entity::BlockEntityKind::BrewingStand(brewing_stand)) => brewing_stand.data(id),
            Some(crate::world::level::block::entity::BlockEntityKind::Crafter(crafter)) => crafter.container_data.get(id),
            _ => 0,
        }
    }

    pub fn menu_container_set_item(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, c: usize, slot: usize, mut stack: ItemStack) {
        match &mut menu.containers[c] {
            MenuContainer::Simple(container) => {
                stack.limit_size(container.get_max_stack_size().min(stack.max_stack_size()));
                container.items[slot] = stack;
                self.menu_container_set_changed(menu, player, c);
            }
            MenuContainer::Crafting(container) => {
                container.items[slot] = stack;
                self.menu_slots_changed(menu, player, c);
            }
            MenuContainer::Result(container) => container.item = stack,
        }
    }

    pub fn menu_container_remove_item(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, c: usize, slot: usize, count: i32) -> ItemStack {
        if let MenuContainer::Result(container) = &mut menu.containers[c] {
            return std::mem::replace(&mut container.item, ItemStack::empty());
        }
        let item = menu.containers[c].get_item_mut(slot);
        let result = if !item.is_empty() && count > 0 { item.split(count) } else { ItemStack::empty() };
        if !result.is_empty() {
            match menu.containers[c] {
                MenuContainer::Simple(_) => self.menu_container_set_changed(menu, player, c),
                _ => self.menu_slots_changed(menu, player, c),
            }
        }
        result
    }

    pub fn menu_container_remove_item_no_update(&mut self, menu: &mut AbstractContainerMenu, c: usize, slot: usize) -> ItemStack {
        std::mem::replace(menu.containers[c].get_item_mut(slot), ItemStack::empty())
    }

    pub fn menu_container_set_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, c: usize) {
        let slots_changed = match menu.containers[c] {
            MenuContainer::Simple(_) if menu.kind == MenuKind::Loom => c == loom_menu::INPUT_CONTAINER,
            MenuContainer::Simple(_) => matches!(menu.kind, MenuKind::Enchantment | MenuKind::Stonecutter | MenuKind::Smithing | MenuKind::Anvil | MenuKind::Grindstone),
            MenuContainer::Result(_) => matches!(menu.kind, MenuKind::Smithing | MenuKind::Anvil),
            MenuContainer::Crafting(_) => false,
        };
        if slots_changed {
            self.menu_slots_changed(menu, player, c);
        }
    }

    pub fn menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, c: usize) {
        match menu.kind {
            MenuKind::Smithing => self.smithing_menu_slots_changed(menu, player, c),
            MenuKind::Anvil => self.item_combiner_menu_slots_changed(menu, player, c),
            MenuKind::Grindstone => self.grindstone_menu_slots_changed(menu, player, c),
            MenuKind::Loom => self.loom_menu_slots_changed(menu, player),
            MenuKind::Enchantment => self.enchantment_menu_slots_changed(menu, player),
            MenuKind::Stonecutter => self.stonecutter_menu_slots_changed(menu, player),
            MenuKind::Inventory => self.slot_changed_crafting_grid(menu, player, None),
            MenuKind::Crafting => {
                if !menu.placing_recipe && menu.access.is_some() {
                    self.slot_changed_crafting_grid(menu, player, None);
                }
            }
            _ => self.menu_broadcast_changes(menu, player),
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
                let carried = self.carried(player);
                if self.can_item_quick_replace(menu, player, i, &carried, true)
                    && self.slot_may_place(menu, player, i, &carried)
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
                    for i in menu.quickcraft_slots.clone() {
                        let carried = self.carried(player);
                        if self.can_item_quick_replace(menu, player, i, &carried, true)
                            && self.slot_may_place(menu, player, i, &carried)
                            && (menu.quickcraft_type == 2 || carried.count >= size as i32)
                        {
                            let current = self.slot_get(menu, player, i);
                            let carry = if current.is_empty() { 0 } else { current.count };
                            let max_size = source.max_stack_size().min(self.slot_max_stack_size(menu, i, &source));
                            let new_count = (quick_craft_place_count(size, menu.quickcraft_type, &source) + carry).min(max_size);
                            remaining -= new_count - carry;
                            self.slot_set_by_player(menu, player, i, source.copy_with_count(new_count));
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
                if !self.slot_may_pickup(menu, player, i) {
                    return Ok(());
                }
                let mut clicked = self.quick_move_stack(menu, player, i);
                while !clicked.is_empty() && ItemStack::is_same_item(&self.slot_get(menu, player, i), &clicked) {
                    clicked = self.quick_move_stack(menu, player, i);
                }
            } else {
                if slot_index < 0 {
                    return Ok(());
                }
                let i = slot_at(menu, slot_index)?;
                let clicked = self.slot_get(menu, player, i);
                let mut carried = self.carried(player);
                if clicked.is_empty() {
                    if !carried.is_empty() {
                        let amount = if primary { carried.count } else { 1 };
                        let rest = self.slot_safe_insert(menu, player, i, carried, amount);
                        self.set_carried(player, rest);
                    }
                } else if self.slot_may_pickup(menu, player, i) {
                    if carried.is_empty() {
                        let amount = if primary { clicked.count } else { (clicked.count + 1) / 2 };
                        if let Some(taken) = self.slot_try_remove(menu, player, i, amount, i32::MAX) {
                            self.set_carried(player, taken.clone());
                            self.slot_on_take(menu, player, i, &taken);
                        }
                    } else if self.slot_may_place(menu, player, i, &carried) {
                        if ItemStack::is_same_item_same_components(&clicked, &carried) {
                            let amount = if primary { carried.count } else { 1 };
                            let rest = self.slot_safe_insert(menu, player, i, carried, amount);
                            self.set_carried(player, rest);
                        } else if carried.count <= self.slot_max_stack_size(menu, i, &carried) {
                            self.set_carried(player, clicked);
                            self.slot_set_by_player(menu, player, i, carried);
                        }
                    } else if ItemStack::is_same_item_same_components(&clicked, &carried)
                        && let Some(taken) = self.slot_try_remove(menu, player, i, clicked.count, carried.max_stack_size() - carried.count)
                    {
                        carried.grow(taken.count);
                        self.set_carried(player, carried);
                        self.slot_on_take(menu, player, i, &taken);
                    }
                }
                self.slot_set_changed(menu, player, i);
            }
        } else if input == SWAP && (0..9).contains(&button) || input == SWAP && button == 40 {
            let inventory_slot = crate::world::item::inventory_slot(button as usize);
            let source = self.player(player).inventory.slots[inventory_slot].clone().unwrap_or_else(ItemStack::empty);
            let i = slot_at(menu, slot_index)?;
            let target_stack = self.slot_get(menu, player, i);
            if !source.is_empty() || !target_stack.is_empty() {
                let set_inventory = |level: &mut ServerLevel, stack: ItemStack| {
                    level.player_mut(player).inventory.slots[inventory_slot] = (!stack.is_empty()).then_some(stack);
                };
                if source.is_empty() {
                    if self.slot_may_pickup(menu, player, i) {
                        set_inventory(self, target_stack.clone());
                        self.slot_on_swap_craft(menu, i, target_stack.count);
                        self.slot_set_by_player(menu, player, i, ItemStack::empty());
                        self.slot_on_take(menu, player, i, &target_stack);
                    }
                } else if target_stack.is_empty() {
                    if self.slot_may_place(menu, player, i, &source) {
                        let max = self.slot_max_stack_size(menu, i, &source);
                        if source.count > max {
                            let mut source = source;
                            let piece = source.split(max);
                            set_inventory(self, source);
                            self.slot_set_by_player(menu, player, i, piece);
                        } else {
                            set_inventory(self, ItemStack::empty());
                            self.slot_set_by_player(menu, player, i, source);
                        }
                    }
                } else if self.slot_may_pickup(menu, player, i) && self.slot_may_place(menu, player, i, &source) {
                    let max = self.slot_max_stack_size(menu, i, &source);
                    if source.count > max {
                        let mut source = source;
                        let piece = source.split(max);
                        set_inventory(self, source);
                        self.slot_set_by_player(menu, player, i, piece);
                        self.slot_on_take(menu, player, i, &target_stack);
                        let mut rest = target_stack;
                        let infinite_materials = self.player(player).abilities.instabuild;
                        self.player_mut(player).inventory.add_stack(&mut rest, infinite_materials);
                        self.drop_from_player_if_any(player, rest);
                    } else {
                        set_inventory(self, target_stack.clone());
                        self.slot_set_by_player(menu, player, i, source);
                        self.slot_on_take(menu, player, i, &target_stack);
                    }
                }
            }
        } else if input == CLONE && self.player(player).abilities.instabuild && self.carried(player).is_empty() && slot_index >= 0 {
            let i = slot_at(menu, slot_index)?;
            if self.slot_has_item(menu, player, i) {
                let clone = self.slot_safe_clone(menu, player, i);
                self.set_carried(player, clone);
            }
        } else if input == THROW && self.carried(player).is_empty() && slot_index >= 0 {
            let i = slot_at(menu, slot_index)?;
            let amount = if button == 0 { 1 } else { self.slot_get(menu, player, i).count };
            let mut stack = self.slot_safe_take(menu, player, i, amount, i32::MAX);
            self.drop_from_player_if_any(player, stack.clone());
            if button == 1 {
                while !stack.is_empty() && ItemStack::is_same_item(&self.slot_get(menu, player, i), &stack) {
                    stack = self.slot_safe_take(menu, player, i, amount, i32::MAX);
                    self.drop_from_player_if_any(player, stack.clone());
                }
            }
        } else if input == PICKUP_ALL && slot_index >= 0 {
            let i = slot_at(menu, slot_index)?;
            let mut carried = self.carried(player);
            if !carried.is_empty() && (!self.slot_has_item(menu, player, i) || !self.slot_may_pickup(menu, player, i)) {
                let len = menu.slots.len() as i32;
                let (start, step) = if button == 0 { (0, 1) } else { (len - 1, -1) };
                for pass in 0..2 {
                    let mut j = start;
                    while j >= 0 && j < len && carried.count < carried.max_stack_size() {
                        let target = j as usize;
                        if self.slot_has_item(menu, player, target)
                            && self.can_item_quick_replace(menu, player, target, &carried, true)
                            && self.slot_may_pickup(menu, player, target)
                            && ServerLevel::can_take_item_for_pick_all(menu, target)
                        {
                            let stack = self.slot_get(menu, player, target);
                            if pass != 0 || stack.count != stack.max_stack_size() {
                                let removed = self.slot_safe_take(menu, player, target, stack.count, carried.max_stack_size() - carried.count);
                                carried.grow(removed.count);
                                self.set_carried(player, carried.clone());
                            }
                        }
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

    pub(super) fn move_item_stack_to(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, stack: &mut ItemStack, start: usize, end: usize, backwards: bool) -> bool {
        let mut anything_changed = false;
        let order: Vec<usize> = if backwards { (start..end).rev().collect() } else { (start..end).collect() };
        if stack.is_stackable() {
            for &i in &order {
                if stack.is_empty() {
                    break;
                }
                let mut target = self.slot_get(menu, player, i);
                if !target.is_empty() && ItemStack::is_same_item_same_components(stack, &target) {
                    let total = target.count + stack.count;
                    let max = self.slot_max_stack_size(menu, i, &target);
                    if total <= max {
                        stack.count = 0;
                        target.count = total;
                        self.slot_put(menu, player, i, target);
                        self.slot_set_changed(menu, player, i);
                        anything_changed = true;
                    } else if target.count < max {
                        stack.shrink(max - target.count);
                        target.count = max;
                        self.slot_put(menu, player, i, target);
                        self.slot_set_changed(menu, player, i);
                        anything_changed = true;
                    }
                }
            }
        }
        if !stack.is_empty() {
            for &i in &order {
                let target = self.slot_get(menu, player, i);
                if target.is_empty() && self.slot_may_place(menu, player, i, stack) {
                    let max = self.slot_max_stack_size(menu, i, stack);
                    let piece = stack.split(stack.count.min(max));
                    self.slot_set_by_player(menu, player, i, piece);
                    self.slot_set_changed(menu, player, i);
                    anything_changed = true;
                    break;
                }
            }
        }
        anything_changed
    }

    fn quick_move_stack(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize) -> ItemStack {
        let mut stack = self.slot_get(menu, player, index);
        if stack.is_empty() {
            return ItemStack::empty();
        }
        let clicked = stack.copy();
        let total = menu.slots.len();
        let moved = match menu.kind {
            MenuKind::Inventory => {
                let equipment_slot = self.get_equipment_slot_for_item(player, &clicked);
                let armor_slot = humanoid_armor_index(equipment_slot).map(|armor_index| 8 - armor_index);
                if index == 0 {
                    let moved = self.move_item_stack_to(menu, player, &mut stack, 9, 45, true);
                    if moved {
                        self.slot_put(menu, player, index, stack.clone());
                        self.slot_on_quick_craft(menu, player, index, &stack, &clicked);
                    }
                    moved
                } else if (1..9).contains(&index) {
                    self.move_item_stack_to(menu, player, &mut stack, 9, 45, false)
                } else if let Some(pos) = armor_slot.filter(|&pos| !self.slot_has_item(menu, player, pos)) {
                    self.move_item_stack_to(menu, player, &mut stack, pos, pos + 1, false)
                } else if equipment_slot == EquipmentSlot::Offhand && !self.slot_has_item(menu, player, 45) {
                    self.move_item_stack_to(menu, player, &mut stack, 45, 46, false)
                } else if (9..36).contains(&index) {
                    self.move_item_stack_to(menu, player, &mut stack, 36, 45, false)
                } else if (36..45).contains(&index) {
                    self.move_item_stack_to(menu, player, &mut stack, 9, 36, false)
                } else {
                    self.move_item_stack_to(menu, player, &mut stack, 9, 45, false)
                }
            }
            MenuKind::Crafting => {
                if index == 0 {
                    let moved = self.move_item_stack_to(menu, player, &mut stack, 10, 46, true);
                    if moved {
                        self.slot_put(menu, player, index, stack.clone());
                        self.slot_on_quick_craft(menu, player, index, &stack, &clicked);
                    }
                    moved
                } else if (10..46).contains(&index) {
                    self.move_item_stack_to(menu, player, &mut stack, 1, 10, false)
                        || if index < 37 { self.move_item_stack_to(menu, player, &mut stack, 37, 46, false) } else { self.move_item_stack_to(menu, player, &mut stack, 10, 37, false) }
                } else {
                    self.move_item_stack_to(menu, player, &mut stack, 10, 46, false)
                }
            }
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
                        self.slot_put(menu, player, index, stack.clone());
                        self.slot_on_quick_craft(menu, player, index, &stack, &clicked);
                    }
                    moved
                } else if index != 1 && index != 0 {
                    if RecipeManager::get().property_set(furnace_allowed_inputs(recipe_type)).test(&stack) {
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
                            self.slot_put(menu, player, index, stack);
                            return ItemStack::empty();
                        }
                        !(self.slot_may_place(menu, player, 3, &stack) && !self.move_item_stack_to(menu, player, &mut stack, 3, 4, false))
                    } else if self.slot_may_place(menu, player, 3, &stack) {
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
                        self.slot_put(menu, player, index, stack.clone());
                        self.slot_on_quick_craft(menu, player, index, &stack, &clicked);
                    }
                    moved
                }
            }
            MenuKind::Lectern => false,
            MenuKind::Smithing | MenuKind::Anvil => self.item_combiner_menu_quick_move_stack(menu, player, index, &mut stack),
            MenuKind::Grindstone => self.grindstone_menu_quick_move_stack(menu, player, index, &mut stack),
            MenuKind::Loom => self.loom_menu_quick_move_stack(menu, player, index, &mut stack),
            MenuKind::Stonecutter => {
                if index == stonecutter_menu::RESULT_SLOT {
                    self.move_item_stack_to(menu, player, &mut stack, stonecutter_menu::INV_SLOT_START, stonecutter_menu::USE_ROW_SLOT_END, true)
                } else if index == stonecutter_menu::INPUT_SLOT {
                    self.move_item_stack_to(menu, player, &mut stack, stonecutter_menu::INV_SLOT_START, stonecutter_menu::USE_ROW_SLOT_END, false)
                } else if RecipeManager::get().stonecutter_recipes().accepts_input(&stack) {
                    self.move_item_stack_to(menu, player, &mut stack, stonecutter_menu::INPUT_SLOT, stonecutter_menu::RESULT_SLOT, false)
                } else if (stonecutter_menu::INV_SLOT_START..stonecutter_menu::INV_SLOT_END).contains(&index) {
                    self.move_item_stack_to(menu, player, &mut stack, stonecutter_menu::USE_ROW_SLOT_START, stonecutter_menu::USE_ROW_SLOT_END, false)
                } else {
                    !((stonecutter_menu::USE_ROW_SLOT_START..stonecutter_menu::USE_ROW_SLOT_END).contains(&index)
                        && !self.move_item_stack_to(menu, player, &mut stack, stonecutter_menu::INV_SLOT_START, stonecutter_menu::INV_SLOT_END, false))
                }
            }
            MenuKind::Crafter => {
                if index < SLOT_COUNT {
                    self.move_item_stack_to(menu, player, &mut stack, INV_SLOT_START, USE_ROW_SLOT_END, true)
                } else {
                    self.move_item_stack_to(menu, player, &mut stack, 0, SLOT_COUNT, false)
                }
            }
            MenuKind::Beacon => {
                if index == PAYMENT_SLOT {
                    let moved = self.move_item_stack_to(menu, player, &mut stack, 1, 37, true);
                    if moved {
                        self.slot_put(menu, player, index, stack.clone());
                        self.slot_on_quick_craft(menu, player, index, &stack, &clicked);
                    }
                    moved
                } else if !self.slot_has_item(menu, player, PAYMENT_SLOT) && self.slot_may_place(menu, player, PAYMENT_SLOT, &stack) && stack.count == 1 {
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
                } else if self.slot_has_item(menu, player, 0) || !self.slot_may_place(menu, player, 0, &stack) {
                    false
                } else {
                    let single_item = stack.copy_with_count(1);
                    stack.shrink(1);
                    self.slot_put(menu, player, index, stack.clone());
                    self.slot_set_by_player(menu, player, 0, single_item);
                    true
                }
            }
        };
        if !moved {
            return ItemStack::empty();
        }
        self.slot_put(menu, player, index, stack.clone());
        if stack.is_empty() {
            self.slot_set_by_player(menu, player, index, ItemStack::empty());
            if menu.kind == MenuKind::Stonecutter {
                self.slot_set_changed(menu, player, index);
            }
        } else {
            self.slot_set_changed(menu, player, index);
        }
        if matches!(
            menu.kind,
            MenuKind::Inventory
                | MenuKind::Crafting
                | MenuKind::Furnace(_)
                | MenuKind::Dispenser
                | MenuKind::BrewingStand
                | MenuKind::Beacon
                | MenuKind::Enchantment
                | MenuKind::Crafter
                | MenuKind::Stonecutter
                | MenuKind::Smithing
                | MenuKind::Anvil
                | MenuKind::Grindstone
                | MenuKind::Loom
        ) {
            if stack.count == clicked.count {
                return ItemStack::empty();
            }
            self.slot_on_take(menu, player, index, &stack);
            match menu.kind {
                MenuKind::Inventory | MenuKind::Crafting if index == 0 => self.player_drop(player, stack, false, Prediction::Predicted),
                MenuKind::Stonecutter => {
                    if index == stonecutter_menu::RESULT_SLOT {
                        self.player_drop(player, stack, false, Prediction::Predicted);
                    }
                    self.menu_broadcast_changes(menu, player);
                }
                _ => {}
            }
        }
        clicked
    }

    fn trigger_slot_listeners(&mut self, menu: &mut AbstractContainerMenu, i: usize, current: &ItemStack) {
        if !ItemStack::matches(&menu.last_slots[i], current) {
            menu.last_slots[i] = current.copy();
            if menu.kind == MenuKind::Crafter {
                self.crafter_menu_slot_changed(menu);
            }
        }
    }

    pub fn menu_broadcast_changes(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        for i in 0..menu.slots.len() {
            let current = self.slot_get(menu, player, i);
            self.trigger_slot_listeners(menu, i, &current);
            if !menu.suppress_remote_updates && !menu.remote_slots[i].matches(&current) {
                menu.remote_slots[i].force(&current);
                if menu.has_synchronizer {
                    let state_id = menu.increment_state_id();
                    self.send_container_set_slot(player, menu.container_id, state_id, i, &current);
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

    pub fn send_container_set_slot(&self, player: EntityId, container_id: i32, state_id: i32, slot: usize, stack: &ItemStack) {
        self.player(player).connection.send(clientbound::CONTAINER_SET_SLOT, |w| {
            w.varint(container_id);
            w.varint(state_id);
            w.i16(slot as i16);
            write_slot(w, Some(stack));
        });
    }

    pub fn menu_broadcast_full_state(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        for i in 0..menu.slots.len() {
            let current = self.slot_get(menu, player, i);
            self.trigger_slot_listeners(menu, i, &current);
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
        let items: Vec<ItemStack> = (0..menu.slots.len()).map(|i| self.slot_get(menu, player, i)).collect();
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
        match menu.kind {
            MenuKind::Inventory => true,
            MenuKind::Crafting => self.still_valid_access(menu.access, player, crisp_data::blocks::CRAFTING_TABLE),
            MenuKind::Beacon => self.still_valid_access(menu.access, player, crisp_data::blocks::BEACON),
            MenuKind::Enchantment => self.still_valid_access(menu.access, player, crisp_data::blocks::ENCHANTING_TABLE),
            MenuKind::Stonecutter => self.still_valid_access(menu.access, player, crisp_data::blocks::STONECUTTER),
            MenuKind::Smithing => self.still_valid_access(menu.access, player, crisp_data::blocks::SMITHING_TABLE),
            MenuKind::Grindstone => self.still_valid_access(menu.access, player, crisp_data::blocks::GRINDSTONE),
            MenuKind::Loom => self.still_valid_access(menu.access, player, crisp_data::blocks::LOOM),
            MenuKind::Anvil => menu.access.is_none_or(|pos| {
                crate::world::level::BlockGetter::block_state(self, pos).block().is(&crisp_data::block_tags::ANVIL) && self.is_within_block_interaction_range(player, pos, 4.0)
            }),
            _ => menu.container.is_none_or(|container| self.container_still_valid(container, player)),
        }
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
        if let Some(container) = menu.container {
            self.container_stop_open(container, player);
        }
        match menu.kind {
            MenuKind::Inventory => {
                menu.containers[1].clear_content();
                self.clear_container(menu, player, 0);
            }
            MenuKind::Crafting => {
                if menu.access.is_some() {
                    self.clear_container(menu, player, 0);
                }
            }
            MenuKind::Beacon => self.beacon_menu_removed(menu, player),
            MenuKind::Enchantment => self.enchantment_menu_removed(menu, player),
            MenuKind::Stonecutter => self.stonecutter_menu_removed(menu, player),
            MenuKind::Smithing | MenuKind::Anvil => self.item_combiner_menu_removed(menu, player),
            MenuKind::Grindstone => self.grindstone_menu_removed(menu, player),
            MenuKind::Loom => self.loom_menu_removed(menu, player),
            _ => {}
        }
    }

    fn drop_or_place_in_inventory(&mut self, player: EntityId, mut carried: ItemStack) {
        let player_removed_not_changing_dimension = self.entity(player).removal.is_some_and(|reason| reason != crate::world::entity::RemovalReason::ChangedDimension);
        if player_removed_not_changing_dimension || self.player(player).disconnected {
            self.player_drop(player, carried, false, Prediction::ServerOnly);
        } else {
            self.place_item_back_in_inventory(player, &mut carried, true);
        }
    }

    pub fn clear_container(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, c: usize) {
        for i in 0..menu.containers[c].get_container_size() {
            let stack = self.menu_container_remove_item_no_update(menu, c, i);
            self.drop_or_place_in_inventory(player, stack);
        }
    }

    pub fn place_item_back_in_inventory(&mut self, player: EntityId, stack: &mut ItemStack, should_send_set_slot_packet: bool) {
        while !stack.is_empty() {
            let inventory = &self.player(player).inventory;
            let Some(slot) = inventory.slot_with_remaining_space(stack).or_else(|| inventory.free_slot()) else {
                self.player_drop(player, stack.clone(), false, Prediction::ServerOnly);
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
            if should_send_set_slot_packet && let Some(index) = crate::world::item::inventory_index(slot) {
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
