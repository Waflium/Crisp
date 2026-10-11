use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::int_limit::IntLimit;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct LimitCount {
    limit: IntLimit,
}

impl LimitCount {
    pub fn decode(tag: &Nbt) -> DataResult<LimitCount> {
        Ok(LimitCount { limit: codec::decode_field(tag, "limit", IntLimit::decode)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        item_stack.count = self.limit.clamp(context, item_stack.count);
        item_stack
    }
}
