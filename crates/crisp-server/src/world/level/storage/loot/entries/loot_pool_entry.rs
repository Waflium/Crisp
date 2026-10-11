use crisp_core::mth;
use crisp_data::Item;

use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::world::item::ItemStack;
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_table::LootTable;

pub enum LootPoolEntryKind<'e> {
    Empty,
    Item(Item),
    Dynamic(&'e str),
    Slots(&'e Holder<SlotSource>),
    Tag(&'e HolderSet),
    LootTable(&'e Holder<LootTable>),
    LootTables(&'e [Holder<LootTable>]),
}

pub struct LootPoolEntry<'e> {
    weight: i32,
    quality: i32,
    kind: LootPoolEntryKind<'e>,
    modifiers: Vec<&'e Holder<LootItemFunction>>,
}

impl<'e> LootPoolEntry<'e> {
    pub fn new(weight: i32, quality: i32, kind: LootPoolEntryKind<'e>, modifiers: &[&'e Holder<LootItemFunction>]) -> LootPoolEntry<'e> {
        LootPoolEntry { weight, quality, kind, modifiers: modifiers.to_vec() }
    }

    pub fn get_weight(&self, luck: f32) -> i32 {
        mth::floor_f(self.weight as f32 + self.quality as f32 * luck).max(0)
    }

    pub fn create_item_stack<'a>(&self, output: &mut dyn FnMut(&mut LootContext<'a>, ItemStack), context: &mut LootContext<'a>) {
        let mut output = |context: &mut LootContext<'a>, mut item_stack: ItemStack| {
            for modifier in &self.modifiers {
                item_stack = modifier.value().apply(item_stack, context);
            }
            output(context, item_stack);
        };
        match &self.kind {
            LootPoolEntryKind::Empty => {}
            LootPoolEntryKind::Item(item) => output(context, ItemStack::new(*item, 1)),
            LootPoolEntryKind::Dynamic(name) => {
                let mut drops = Vec::new();
                context.add_dynamic_drops(name, &mut |item_stack| drops.push(item_stack));
                for item_stack in drops {
                    output(context, item_stack);
                }
            }
            LootPoolEntryKind::Slots(slot_source) => {
                for item_stack in slot_source.value().provide(context).item_copies() {
                    if !item_stack.is_empty() {
                        output(context, item_stack);
                    }
                }
            }
            LootPoolEntryKind::Tag(tag) => {
                for &item in tag.contents() {
                    output(context, ItemStack::new(Item(item as u16), 1));
                }
            }
            LootPoolEntryKind::LootTable(table) => table.value().get_random_items_raw(context, &mut output),
            LootPoolEntryKind::LootTables(tables) => {
                for table in *tables {
                    table.value().get_random_items_raw(context, &mut output);
                }
            }
        }
    }
}
