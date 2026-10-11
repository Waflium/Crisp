use crisp_protocol::nbt::Nbt;

use super::loot_pool_entry::{LootPoolEntry, LootPoolEntryKind};
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;

pub const DEFAULT_WEIGHT: i32 = 1;
pub const DEFAULT_QUALITY: i32 = 0;

pub struct UniformContainerBase {
    weight: i32,
    quality: i32,
}

impl UniformContainerBase {
    pub fn decode(tag: &Nbt) -> DataResult<UniformContainerBase> {
        Ok(UniformContainerBase {
            weight: codec::decode_or(tag, "weight", DEFAULT_WEIGHT, codec::int)?,
            quality: codec::decode_or(tag, "quality", DEFAULT_QUALITY, codec::int)?,
        })
    }

    pub fn entry<'e>(&self, kind: LootPoolEntryKind<'e>, modifiers: &[&'e Holder<LootItemFunction>]) -> LootPoolEntry<'e> {
        LootPoolEntry::new(self.weight, self.quality, kind, modifiers)
    }
}
