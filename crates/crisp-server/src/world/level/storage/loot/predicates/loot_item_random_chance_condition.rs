use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;

pub struct LootItemRandomChanceCondition {
    chance: Holder<ContextFloatProvider>,
}

impl LootItemRandomChanceCondition {
    pub fn decode(tag: &Nbt) -> DataResult<LootItemRandomChanceCondition> {
        Ok(LootItemRandomChanceCondition { chance: codec::decode_field(tag, "chance", ContextFloatProvider::decode_holder)? })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let probability = self.chance.value().get_float(context);
        context.random().next_float() < probability
    }
}
