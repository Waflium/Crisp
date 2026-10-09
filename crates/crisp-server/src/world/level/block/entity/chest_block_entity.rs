use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::container_openers_counter::ContainerOpenersCounter;
use crate::world::item::ItemStack;

pub const EVENT_SET_OPEN_COUNT: i32 = 1;

#[derive(Clone)]
pub struct ChestBlockEntity {
    pub base: BaseContainerBlockEntity,
    pub openers_counter: ContainerOpenersCounter,
    pub lid_should_be_open: bool,
}

impl Default for ChestBlockEntity {
    fn default() -> ChestBlockEntity {
        ChestBlockEntity { base: BaseContainerBlockEntity::with_size(27), openers_counter: ContainerOpenersCounter::default(), lid_should_be_open: false }
    }
}

impl ChestBlockEntity {
    pub const CONTAINER_SIZE: usize = 27;

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.base.items = vec![ItemStack::empty(); Self::CONTAINER_SIZE];
        if !self.base.try_load_loot_table(input) {
            self.base.load_all_items(input, Self::CONTAINER_SIZE);
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        if !self.base.try_save_loot_table(output) {
            save_all_items(output, &self.base.items, true);
        }
    }

    pub fn trigger_event(&mut self, b0: i32, b1: i32) -> bool {
        if b0 == EVENT_SET_OPEN_COUNT {
            self.lid_should_be_open = b1 > 0;
            true
        } else {
            false
        }
    }
}
