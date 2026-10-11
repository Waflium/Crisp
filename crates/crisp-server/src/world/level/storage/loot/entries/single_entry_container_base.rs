use crisp_data::Item;
use crisp_protocol::nbt::Nbt;

use super::loot_pool_entry::{LootPoolEntry, LootPoolEntryKind};
use super::uniform_container_base::UniformContainerBase;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContext;

pub enum SingleEntry {
    EmptyLootItem,
    LootItem(Item),
    DynamicLoot(String),
    SlotLoot(Holder<SlotSource>),
}

pub struct SingleEntryContainerBase {
    base: UniformContainerBase,
    entry: SingleEntry,
}

impl SingleEntryContainerBase {
    pub fn decode(tag: &Nbt, ty: &str) -> DataResult<SingleEntryContainerBase> {
        let entry = match ty {
            "minecraft:item" => SingleEntry::LootItem(codec::decode_field(tag, "name", |t| {
                let id = codec::identifier(t)?;
                Item::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: {id}"))
            })?),
            "minecraft:dynamic" => SingleEntry::DynamicLoot(codec::decode_field(tag, "name", codec::identifier)?),
            "minecraft:slots" => SingleEntry::SlotLoot(codec::decode_field(tag, "slot_source", SlotSource::decode_holder)?),
            _ => SingleEntry::EmptyLootItem,
        };
        Ok(SingleEntryContainerBase { base: UniformContainerBase::decode(tag)?, entry })
    }

    pub fn expand_raw<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        let kind = match &self.entry {
            SingleEntry::EmptyLootItem => LootPoolEntryKind::Empty,
            SingleEntry::LootItem(item) => LootPoolEntryKind::Item(*item),
            SingleEntry::DynamicLoot(name) => LootPoolEntryKind::Dynamic(name),
            SingleEntry::SlotLoot(slot_source) => LootPoolEntryKind::Slots(slot_source),
        };
        output(context, self.base.entry(kind, modifiers));
        true
    }
}
