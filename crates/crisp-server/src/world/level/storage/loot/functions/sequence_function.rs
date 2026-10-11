use crisp_protocol::nbt::Nbt;

use super::loot_item_function::LootItemFunction;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SequenceFunction {
    functions: Vec<Holder<LootItemFunction>>,
}

impl SequenceFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SequenceFunction> {
        Ok(SequenceFunction { functions: codec::decode_field(tag, "functions", LootItemFunction::decode_list)? })
    }

    pub fn decode_inline(tag: &Nbt) -> DataResult<SequenceFunction> {
        Ok(SequenceFunction { functions: LootItemFunction::decode_list(tag)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        for function in &self.functions {
            item_stack = function.value().apply(item_stack, context);
        }
        item_stack
    }
}
