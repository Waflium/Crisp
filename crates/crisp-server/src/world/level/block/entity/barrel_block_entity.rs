use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::container_openers_counter::ContainerOpenersCounter;
use crate::world::item::ItemStack;

#[derive(Clone)]
pub struct BarrelBlockEntity {
    pub base: BaseContainerBlockEntity,
    pub openers_counter: ContainerOpenersCounter,
}

impl Default for BarrelBlockEntity {
    fn default() -> BarrelBlockEntity {
        BarrelBlockEntity { base: BaseContainerBlockEntity::with_size(27), openers_counter: ContainerOpenersCounter::default() }
    }
}

impl BarrelBlockEntity {
    pub const CONTAINER_SIZE: usize = 27;

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        if !self.base.try_save_loot_table(output) {
            save_all_items(output, &self.base.items, true);
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.base.items = vec![ItemStack::empty(); Self::CONTAINER_SIZE];
        if !self.base.try_load_loot_table(input) {
            self.base.load_all_items(input, Self::CONTAINER_SIZE);
        }
    }
}
