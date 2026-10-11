use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::float_range_predicate::FloatRangePredicate;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;

pub struct FloatValueCheck {
    value: Holder<ContextFloatProvider>,
    range: FloatRangePredicate,
}

impl FloatValueCheck {
    pub fn decode(tag: &Nbt) -> DataResult<FloatValueCheck> {
        Ok(FloatValueCheck {
            value: codec::decode_field(tag, "value", ContextFloatProvider::decode_holder)?,
            range: codec::decode_field(tag, "test", FloatRangePredicate::decode)?,
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.range.test_provider(context, self.value.value())
    }
}
