use crisp_protocol::nbt::Nbt;

use super::item_predicate::ItemPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::inventory::slot_ranges::{self, SlotRange};
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct SlotsPredicate {
    slots: Vec<(&'static SlotRange, ItemPredicate)>,
}

impl SlotsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<SlotsPredicate> {
        let slots = codec::unbounded_map(tag, |name| slot_ranges::name_to_ids(name).ok_or_else(|| format!("Unknown element name:{name}")), ItemPredicate::decode)?;
        Ok(SlotsPredicate { slots })
    }

    pub fn matches(&self, slot_provider: impl Fn(i32) -> Option<ItemStack>) -> bool {
        self.slots.iter().all(|(range, test)| range.slots.iter().any(|&slot_id| slot_provider(slot_id).is_some_and(|stack| test.test(&stack))))
    }
}
