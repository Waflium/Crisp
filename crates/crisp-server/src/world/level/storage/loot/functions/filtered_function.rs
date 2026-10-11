use crisp_protocol::nbt::Nbt;

use super::loot_item_function::LootItemFunction;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct FilteredFunction {
    filter: ItemPredicate,
    on_pass: Option<Holder<LootItemFunction>>,
    on_fail: Option<Holder<LootItemFunction>>,
}

impl FilteredFunction {
    pub fn decode(tag: &Nbt) -> DataResult<FilteredFunction> {
        Ok(FilteredFunction {
            filter: codec::decode_field(tag, "item_filter", ItemPredicate::decode)?,
            on_pass: codec::decode_optional(tag, "on_pass", LootItemFunction::decode_holder)?,
            on_fail: codec::decode_optional(tag, "on_fail", LootItemFunction::decode_holder)?,
        })
    }

    pub fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let function = if self.filter.test(&item_stack) { &self.on_pass } else { &self.on_fail };
        match function {
            Some(function) => function.value().apply(item_stack, context),
            None => item_stack,
        }
    }
}
