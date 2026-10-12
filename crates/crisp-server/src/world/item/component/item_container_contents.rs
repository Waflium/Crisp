use crisp_protocol::nbt::Nbt;

use super::growable_mutable_container::GrowableMutableContainer;
use super::simple_mutable_container::SimpleMutableContainer;
use super::values;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::base_container_block_entity;

pub const MAX_SIZE: usize = 256;

pub fn item_copies(contents: &Nbt) -> Vec<ItemStack> {
    let slots = values::container_slots(contents);
    let Some(max_slot_index) = slots.iter().map(|&(index, _)| index).max() else { return Vec::new() };
    let mut items = vec![ItemStack::empty(); max_slot_index as usize + 1];
    for (index, template) in &slots {
        items[*index as usize] = template.create().unwrap_or_else(ItemStack::empty);
    }
    items
}

pub fn as_mutable(contents: &Nbt) -> Mutable {
    Mutable { items: item_copies(contents) }
}

pub struct Mutable {
    items: Vec<ItemStack>,
}

impl Mutable {
    pub fn to_immutable(&self) -> Nbt {
        base_container_block_entity::from_items(&self.items)
    }
}

impl SimpleMutableContainer for Mutable {
    fn items(&mut self) -> &mut Vec<ItemStack> {
        &mut self.items
    }
}

impl GrowableMutableContainer for Mutable {
    fn can_insert_new_slots(&mut self) -> bool {
        self.items.len() < MAX_SIZE
    }
}
