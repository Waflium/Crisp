use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct ContainerPredicate {
    items: Option<CollectionPredicate<ItemPredicate>>,
}

impl ContainerPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<ContainerPredicate> {
        codec::map(tag)?;
        Ok(ContainerPredicate { items: codec::decode_optional(tag, "items", |t| CollectionPredicate::decode(t, ItemPredicate::decode))? })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let Some(items) = &self.items else { return true };
        let non_empty_items: Vec<ItemStack> =
            value.as_list().iter().filter_map(|slot| slot.get("item")).filter_map(|item| ItemStack::from_nbt(item).ok()).filter(|stack| !stack.is_empty()).collect();
        items.test(&non_empty_items, |p, stack| p.test(stack))
    }
}
