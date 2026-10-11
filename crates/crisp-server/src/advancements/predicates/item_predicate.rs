use crisp_protocol::nbt::Nbt;

use super::data_component_matchers::DataComponentMatchers;
use super::min_max_bounds::Ints;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct ItemPredicate {
    items: Option<HolderSet>,
    count: Ints,
    components: DataComponentMatchers,
}

impl ItemPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<ItemPredicate> {
        codec::map(tag)?;
        Ok(ItemPredicate {
            items: codec::decode_optional(tag, "items", |t| HolderSet::decode("minecraft:item", t))?,
            count: codec::decode_or(tag, "count", Ints::ANY, Ints::decode)?,
            components: DataComponentMatchers::decode(tag)?,
        })
    }

    pub fn test(&self, item_stack: &ItemStack) -> bool {
        if self.items.as_ref().is_some_and(|items| !items.contains(item_stack.item.0 as i32)) {
            return false;
        }
        self.count.matches(item_stack.count) && self.components.test(item_stack)
    }
}
