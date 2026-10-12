use crisp_protocol::nbt::Nbt;

use super::growable_mutable_container::GrowableMutableContainer;
use super::simple_mutable_container::SimpleMutableContainer;
use super::bundle_contents;
use crate::world::item::ItemStack;
use crate::world::item::item_provider::ItemProvider;

pub fn as_mutable(contents: &Nbt) -> Mutable {
    Mutable { items: bundle_contents::item_copies(contents) }
}

pub struct Mutable {
    items: Vec<ItemStack>,
}

impl Mutable {
    pub fn to_immutable(&self) -> Nbt {
        Nbt::List(self.items.iter().filter(|item| !item.is_empty()).map(bundle_contents::template_of).collect())
    }
}

impl SimpleMutableContainer for Mutable {
    fn items(&mut self) -> &mut Vec<ItemStack> {
        &mut self.items
    }
}

impl GrowableMutableContainer for Mutable {
    fn can_insert_new_slots(&mut self) -> bool {
        self.items.len() < 1024
    }

    fn add_slot_with_item(&mut self, new_items: &mut dyn ItemProvider) -> bool {
        if !new_items.find_next_non_empty() {
            return false;
        }
        let next = new_items.next();
        self.items.push(next);
        true
    }
}
