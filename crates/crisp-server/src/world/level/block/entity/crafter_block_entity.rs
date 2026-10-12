use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::properties::{CRAFTING, TRIGGERED};
use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::{BlockEntityKind, int_or};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::crafting::crafting_input::CraftingInput;
use crate::world::level::update_flags::UPDATE_ALL;

pub const CONTAINER_WIDTH: usize = 3;
pub const CONTAINER_HEIGHT: usize = 3;
pub const CONTAINER_SIZE: usize = 9;
pub const SLOT_DISABLED: i32 = 1;
pub const SLOT_ENABLED: i32 = 0;
pub const DATA_TRIGGERED: usize = 9;
pub const NUM_DATA: usize = 10;

#[derive(Clone, Default)]
pub struct ContainerData {
    slot_states: [i32; CONTAINER_SIZE],
    triggered: i32,
}

impl ContainerData {
    pub fn get(&self, data_id: usize) -> i32 {
        if data_id == DATA_TRIGGERED { self.triggered } else { self.slot_states[data_id] }
    }

    pub fn set(&mut self, data_id: usize, value: i32) {
        if data_id == DATA_TRIGGERED {
            self.triggered = value;
        } else {
            self.slot_states[data_id] = value;
        }
    }
}

#[derive(Clone)]
pub struct CrafterBlockEntity {
    pub base: BaseContainerBlockEntity,
    crafting_ticks_remaining: i32,
    pub container_data: ContainerData,
}

impl CrafterBlockEntity {
    pub fn new(state: BlockState) -> CrafterBlockEntity {
        let mut crafter_block_entity = CrafterBlockEntity { base: BaseContainerBlockEntity::with_size(CONTAINER_SIZE), crafting_ticks_remaining: 0, container_data: ContainerData::default() };
        crafter_block_entity.set_triggered(state.get(TRIGGERED));
        crafter_block_entity
    }

    pub fn set_slot_state(&mut self, slot_id: i32, enabled: bool) -> bool {
        if self.slot_can_be_disabled(slot_id) {
            self.container_data.set(slot_id as usize, if enabled { SLOT_ENABLED } else { SLOT_DISABLED });
            return true;
        }
        false
    }

    pub fn is_slot_disabled(&self, slot_id: i32) -> bool {
        if (0..CONTAINER_SIZE as i32).contains(&slot_id) { self.container_data.get(slot_id as usize) == SLOT_DISABLED } else { false }
    }

    pub fn can_place_item(&self, slot: usize, _item_stack: &ItemStack) -> bool {
        if self.container_data.get(slot) == SLOT_DISABLED {
            return false;
        }
        let slot_stack = &self.base.items[slot];
        let current_stack_size = slot_stack.count;
        if current_stack_size >= slot_stack.max_stack_size() {
            return false;
        }
        if slot_stack.is_empty() { true } else { !self.smaller_stack_exist(current_stack_size, slot_stack, slot) }
    }

    fn smaller_stack_exist(&self, base_size: i32, base_item: &ItemStack, base_slot: usize) -> bool {
        for i in base_slot + 1..CONTAINER_SIZE {
            if !self.is_slot_disabled(i as i32) {
                let slot_stack = self.get_item(i);
                if slot_stack.is_empty() || slot_stack.count < base_size && ItemStack::is_same_item_same_components(slot_stack, base_item) {
                    return true;
                }
            }
        }
        false
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.crafting_ticks_remaining = int_or(input, "crafting_ticks_remaining", 0);
        self.base.items = vec![ItemStack::empty(); CONTAINER_SIZE];
        if !self.base.try_load_loot_table(input) {
            self.base.load_all_items(input, CONTAINER_SIZE);
        }
        for i in 0..CONTAINER_SIZE {
            self.container_data.set(i, SLOT_ENABLED);
        }
        if let Some(Nbt::IntArray(disabled_slots)) = input.get("disabled_slots") {
            for &i in disabled_slots {
                if self.slot_can_be_disabled(i) {
                    self.container_data.set(i as usize, SLOT_DISABLED);
                }
            }
        }
        self.container_data.set(DATA_TRIGGERED, int_or(input, "triggered", 0));
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        output.push(("crafting_ticks_remaining".into(), Nbt::Int(self.crafting_ticks_remaining)));
        if !self.base.try_save_loot_table(output) {
            save_all_items(output, &self.base.items, true);
        }
        self.add_disabled_slots(output);
        self.add_triggered(output);
    }

    pub fn is_empty(&self) -> bool {
        self.base.items.iter().all(ItemStack::is_empty)
    }

    pub fn get_item(&self, slot: usize) -> &ItemStack {
        &self.base.items[slot]
    }

    pub fn get_items_mut(&mut self) -> &mut Vec<ItemStack> {
        &mut self.base.items
    }

    pub fn as_craft_input(&self) -> CraftingInput {
        CraftingInput::of_positioned(CONTAINER_WIDTH, CONTAINER_HEIGHT, &self.base.items).input
    }

    fn add_disabled_slots(&self, output: &mut Vec<(String, Nbt)>) {
        let disabled_slots = (0..CONTAINER_SIZE as i32).filter(|&i| self.is_slot_disabled(i)).collect();
        output.push(("disabled_slots".into(), Nbt::IntArray(disabled_slots)));
    }

    fn add_triggered(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("triggered".into(), Nbt::Int(self.container_data.get(DATA_TRIGGERED))));
    }

    pub fn set_triggered(&mut self, value: bool) {
        self.container_data.set(DATA_TRIGGERED, if value { 1 } else { 0 });
    }

    pub fn set_crafting_ticks_remaining(&mut self, max_crafting_ticks: i32) {
        self.crafting_ticks_remaining = max_crafting_ticks;
    }

    pub fn get_redstone_signal(&self) -> i32 {
        (0..CONTAINER_SIZE).filter(|&i| !self.get_item(i).is_empty() || self.is_slot_disabled(i as i32)).count() as i32
    }

    fn slot_can_be_disabled(&self, slot_id: i32) -> bool {
        slot_id > -1 && slot_id < CONTAINER_SIZE as i32 && self.base.items[slot_id as usize].is_empty()
    }
}

impl ServerLevel {
    pub fn crafter(&mut self, pos: BlockPos) -> Option<&mut CrafterBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Crafter(crafter) => Some(crafter),
            _ => None,
        }
    }

    pub fn crafter_set_slot_state(&mut self, pos: BlockPos, slot_id: i32, enabled: bool) {
        if self.crafter(pos).is_some_and(|crafter| crafter.set_slot_state(slot_id, enabled)) {
            self.block_entity_set_changed(pos);
        }
    }

    pub fn crafter_server_tick(&mut self, pos: BlockPos, state: BlockState) {
        let Some(entity) = self.crafter(pos) else { return };
        let crafting_ticks_remaining = entity.crafting_ticks_remaining - 1;
        if crafting_ticks_remaining >= 0 {
            entity.crafting_ticks_remaining = crafting_ticks_remaining;
            if crafting_ticks_remaining == 0 {
                self.set_block(pos, state.set(CRAFTING, false), UPDATE_ALL);
            }
        }
    }
}
