use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct BundlePredicate {
    items: Option<CollectionPredicate<ItemPredicate>>,
}

impl BundlePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<BundlePredicate> {
        codec::map(tag)?;
        Ok(BundlePredicate { items: codec::decode_optional(tag, "items", |t| CollectionPredicate::decode(t, ItemPredicate::decode))? })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let Some(items) = &self.items else { return true };
        let stacks: Vec<ItemStack> = value.as_list().iter().filter_map(|item| ItemStack::from_nbt(item).ok()).collect();
        items.test(&stacks, |p, stack| p.test(stack))
    }
}
