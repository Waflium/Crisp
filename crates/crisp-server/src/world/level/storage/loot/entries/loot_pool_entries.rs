use crisp_protocol::nbt::Nbt;

use super::composite_entry_base::CompositeEntryBase;
use super::expandable_container_base::ExpandableContainerBase;
use super::loot_pool_entry_container::{LootPoolEntryContainer, LootPoolEntryContainerType};
use super::single_entry_container_base::SingleEntryContainerBase;
use crate::util::codec::{self, DataResult};

pub fn decode(tag: &Nbt) -> DataResult<LootPoolEntryContainer> {
    codec::map(tag)?;
    let ty = codec::decode_field(tag, "type", codec::identifier)?;
    let entry = match ty.as_str() {
        "minecraft:empty" | "minecraft:item" | "minecraft:dynamic" | "minecraft:slots" => LootPoolEntryContainerType::Single(SingleEntryContainerBase::decode(tag, &ty)?),
        "minecraft:loot_table" | "minecraft:tag" => LootPoolEntryContainerType::Expandable(ExpandableContainerBase::decode(tag, &ty)?),
        "minecraft:alternatives" | "minecraft:sequence" | "minecraft:group" => LootPoolEntryContainerType::Composite(CompositeEntryBase::decode(tag, &ty)?),
        other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_pool_entry_type]: {other}")),
    };
    LootPoolEntryContainer::decode(tag, entry)
}
