use crisp_data::Item;
use crisp_protocol::nbt::Nbt;

use super::loot_pool_entry::{LootPoolEntry, LootPoolEntryKind};
use super::uniform_container_base::UniformContainerBase;
use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_table::LootTable;

pub enum ExpandableEntry {
    TagEntry(HolderSet),
    NestedLootTable(Vec<Holder<LootTable>>),
}

pub struct ExpandableContainerBase {
    base: UniformContainerBase,
    expand: bool,
    entry: ExpandableEntry,
}

impl ExpandableContainerBase {
    pub fn decode(tag: &Nbt, ty: &str) -> DataResult<ExpandableContainerBase> {
        let entry = match ty {
            "minecraft:tag" => ExpandableEntry::TagEntry(codec::decode_field(tag, "items", |t| HolderSet::decode("minecraft:item", t))?),
            _ => ExpandableEntry::NestedLootTable(codec::decode_field(tag, "value", LootTable::decode_list)?),
        };
        Ok(ExpandableContainerBase { base: UniformContainerBase::decode(tag)?, expand: codec::decode_or(tag, "expand", false, codec::bool_value)?, entry })
    }

    pub fn expand_raw<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        if self.expand { self.add_expanded_entries(context, modifiers, output) } else { self.add_unexpanded_entry(context, modifiers, output) }
    }

    fn add_expanded_entries<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        match &self.entry {
            ExpandableEntry::TagEntry(tag) => {
                for &item in tag.contents() {
                    output(context, self.base.entry(LootPoolEntryKind::Item(Item(item as u16)), modifiers));
                }
            }
            ExpandableEntry::NestedLootTable(value) => {
                for table in value {
                    output(context, self.base.entry(LootPoolEntryKind::LootTable(table), modifiers));
                }
            }
        }
        true
    }

    fn add_unexpanded_entry<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        let kind = match &self.entry {
            ExpandableEntry::TagEntry(tag) => LootPoolEntryKind::Tag(tag),
            ExpandableEntry::NestedLootTable(value) => LootPoolEntryKind::LootTables(value),
        };
        output(context, self.base.entry(kind, modifiers));
        true
    }
}
