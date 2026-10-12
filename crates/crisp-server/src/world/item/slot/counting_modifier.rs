use crate::server::level::ServerLevel;
use crate::world::entity::slot_access::SlotAccess;
use crate::world::item::ItemStack;

pub struct CountingModifier<'m> {
    modifier: &'m mut dyn FnMut(&mut ServerLevel, ItemStack) -> ItemStack,
    updated_count: i32,
}

impl<'m> CountingModifier<'m> {
    pub fn new(modifier: &'m mut dyn FnMut(&mut ServerLevel, ItemStack) -> ItemStack) -> CountingModifier<'m> {
        CountingModifier { modifier, updated_count: 0 }
    }

    pub fn accept(&mut self, level: &mut ServerLevel, slot: &mut dyn SlotAccess) {
        let item = slot.get(level);
        let new_item = (self.modifier)(level, item);
        let success = slot.set(level, new_item);
        if success {
            self.updated_count += 1;
        }
    }

    pub fn updated_count(&self) -> i32 {
        self.updated_count
    }
}
