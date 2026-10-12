use super::simple_mutable_container::{self, SimpleMutableContainer};
use crate::world::item::ItemStack;
use crate::world::item::item_provider::ItemProvider;
use crate::world::item::slot::slot_selector::SlotSelector;

pub trait GrowableMutableContainer: SimpleMutableContainer {
    fn can_insert_new_slots(&mut self) -> bool;

    fn add_slot_with_item(&mut self, new_items: &mut dyn ItemProvider) -> bool {
        let next = new_items.next();
        self.items().push(next);
        true
    }
}

pub fn growable_replace_slot_items<C: GrowableMutableContainer + ?Sized>(container: &mut C, new_items: &mut dyn ItemProvider, slot_selector: &SlotSelector) -> i32 {
    let mut success_count = simple_mutable_container::simple_replace_slot_items(container, new_items, slot_selector);
    while new_items.has_next() && container.can_insert_new_slots() && slot_selector.try_select_slot(&ItemStack::empty()) && container.add_slot_with_item(new_items) {
        success_count += 1;
    }
    success_count
}
