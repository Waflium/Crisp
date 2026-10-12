use crate::server::level::ServerLevel;
use crate::world::entity::slot_access::SlotAccess;
use crate::world::item::ItemStack;
use crate::world::item::item_provider::ItemProvider;
use crate::world::item::slot::slot_selector::SlotSelector;

pub type SlotConsumer<'c> = dyn FnMut(&mut ServerLevel, &mut dyn SlotAccess) + 'c;

pub trait SimpleMutableContainer {
    fn items(&mut self) -> &mut Vec<ItemStack>;

    fn set_item(&mut self, slot: usize, item_stack: ItemStack) -> bool {
        self.items()[slot] = item_stack;
        true
    }
}

pub fn simple_replace_slot_items<C: SimpleMutableContainer + ?Sized>(container: &mut C, new_items: &mut dyn ItemProvider, slot_selector: &SlotSelector) -> i32 {
    let mut success_count = 0;
    let mut index = 0;
    while index < container.items().len() && new_items.has_next() {
        let current_item = container.items()[index].clone();
        if slot_selector.try_select_slot(&current_item) {
            let success = container.set_item(index, new_items.next());
            if success {
                success_count += 1;
            }
        }
        index += 1;
    }
    success_count
}

pub fn simple_modify_slots<C: SimpleMutableContainer + ?Sized>(container: &mut C, level: &mut ServerLevel, consumer: &mut SlotConsumer, slot_selector: &SlotSelector) {
    let mut i = 0;
    while i < container.items().len() {
        let current_item = container.items()[i].clone();
        if slot_selector.try_select_slot(&current_item) {
            consumer(level, &mut ListSlotAccess { container: &mut *container, slot: i });
        }
        i += 1;
    }
}

struct ListSlotAccess<'m, C: SimpleMutableContainer + ?Sized> {
    container: &'m mut C,
    slot: usize,
}

impl<C: SimpleMutableContainer + ?Sized> SlotAccess for ListSlotAccess<'_, C> {
    fn get(&mut self, _level: &mut ServerLevel) -> ItemStack {
        self.container.items()[self.slot].clone()
    }

    fn set(&mut self, _level: &mut ServerLevel, item_stack: ItemStack) -> bool {
        self.container.set_item(self.slot, item_stack);
        true
    }

    fn modify(&mut self, level: &mut ServerLevel, modifier: &mut dyn FnMut(&mut ServerLevel, &mut ItemStack)) {
        modifier(level, &mut self.container.items()[self.slot]);
    }
}
