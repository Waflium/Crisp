use crisp_protocol::nbt::Nbt;

use super::loot_item_condition::LootItemCondition;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct InvertedLootItemCondition {
    term: Holder<LootItemCondition>,
}

impl InvertedLootItemCondition {
    pub fn decode(tag: &Nbt) -> DataResult<InvertedLootItemCondition> {
        Ok(InvertedLootItemCondition { term: codec::decode_field(tag, "term", LootItemCondition::decode_holder)? })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        !self.term.value().test(context)
    }
}
