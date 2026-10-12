use crisp_data::item_tags;

use crate::world::item::ItemStack;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SimpleContainerKind {
    #[default]
    Plain,
    BeaconPayment,
}

#[derive(Clone, Debug, Default)]
pub struct SimpleContainer {
    pub kind: SimpleContainerKind,
    pub items: Vec<ItemStack>,
    pub changed: bool,
}

impl SimpleContainer {
    pub fn new(kind: SimpleContainerKind, size: usize) -> SimpleContainer {
        SimpleContainer { kind, items: vec![ItemStack::empty(); size], changed: false }
    }

    pub fn get_max_stack_size(&self) -> i32 {
        match self.kind {
            SimpleContainerKind::Plain => 99,
            SimpleContainerKind::BeaconPayment => 1,
        }
    }

    pub fn can_place_item(&self, _slot: usize, stack: &ItemStack) -> bool {
        match self.kind {
            SimpleContainerKind::Plain => true,
            SimpleContainerKind::BeaconPayment => stack.get_item().is(&item_tags::BEACON_PAYMENT_ITEMS),
        }
    }

    pub fn set_changed(&mut self) {
        self.changed = true;
    }
}
