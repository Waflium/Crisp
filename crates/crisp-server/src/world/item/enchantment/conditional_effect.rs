use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

pub struct ConditionalEffect<T> {
    pub effect: T,
    pub requirements: Option<Holder<LootItemCondition>>,
}

impl<T> ConditionalEffect<T> {
    pub fn decode(tag: &Nbt, effect: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<ConditionalEffect<T>> {
        Ok(ConditionalEffect {
            effect: codec::decode_field(tag, "effect", effect)?,
            requirements: codec::decode_optional(tag, "requirements", LootItemCondition::decode_holder)?,
        })
    }

    pub fn matches(&self, context: &mut LootContext) -> bool {
        self.requirements.as_ref().is_none_or(|requirements| requirements.value().test(context))
    }
}
