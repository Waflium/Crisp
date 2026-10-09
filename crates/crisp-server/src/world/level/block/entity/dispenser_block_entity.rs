use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use crate::world::item::ItemStack;

#[derive(Clone)]
pub struct DispenserBlockEntity {
    pub base: BaseContainerBlockEntity,
}

impl Default for DispenserBlockEntity {
    fn default() -> DispenserBlockEntity {
        DispenserBlockEntity { base: BaseContainerBlockEntity::with_size(9) }
    }
}

impl DispenserBlockEntity {
    pub const CONTAINER_SIZE: usize = 9;

    pub fn get_random_slot(&self, random: &mut impl RandomSource) -> i32 {
        let mut replace_slot = -1;
        let mut replace_odds = 1;
        for (i, stack) in self.base.items.iter().enumerate() {
            if !stack.is_empty() {
                let odds = replace_odds;
                replace_odds += 1;
                if random.next_int_bounded(odds) == 0 {
                    replace_slot = i as i32;
                }
            }
        }
        replace_slot
    }

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
}
