use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub struct SetItemCountFunction {
    count: Holder<ContextIntProvider>,
    add: bool,
}

impl SetItemCountFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetItemCountFunction> {
        Ok(SetItemCountFunction {
            count: codec::decode_field(tag, "count", ContextIntProvider::decode_holder)?,
            add: codec::decode_or(tag, "add", false, codec::bool_value)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let base = if self.add { item_stack.count } else { 0 };
        item_stack.count = base.wrapping_add(self.count.value().get_int(context));
        item_stack
    }
}
