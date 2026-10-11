use crisp_core::mth;
use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use super::entries::loot_pool_entries;
use super::entries::loot_pool_entry::LootPoolEntry;
use super::entries::loot_pool_entry_container::LootPoolEntryContainer;
use super::functions::loot_item_function::{self, LootItemFunction};
use super::loot_context::LootContext;
use super::predicates::loot_item_condition::LootItemCondition;
use super::providers::number::floats::{self, ContextFloatProvider};
use super::providers::number::ints::ContextIntProvider;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;

pub struct LootPool {
    entries: Vec<LootPoolEntryContainer>,
    condition: Option<Holder<LootItemCondition>>,
    modifier: Option<Holder<LootItemFunction>>,
    rolls: Holder<ContextIntProvider>,
    bonus_rolls: Holder<ContextFloatProvider>,
}

impl LootPool {
    pub fn decode(tag: &Nbt) -> DataResult<LootPool> {
        codec::map(tag)?;
        Ok(LootPool {
            entries: codec::decode_field(tag, "entries", |t| codec::list_of(t, loot_pool_entries::decode))?,
            condition: codec::decode_optional(tag, "condition", LootItemCondition::decode_holder)?,
            modifier: codec::decode_optional(tag, "modifier", LootItemFunction::decode_holder)?,
            rolls: codec::decode_field(tag, "rolls", ContextIntProvider::decode_holder)?,
            bonus_rolls: codec::decode_or(tag, "bonus_rolls", floats::exactly(0.0), ContextFloatProvider::decode_holder)?,
        })
    }

    fn add_random_item<'a>(&self, result: &mut dyn FnMut(&mut LootContext<'a>, ItemStack), context: &mut LootContext<'a>) {
        let mut valid_entries: Vec<LootPoolEntry> = Vec::new();
        let mut total_weight = 0;
        for entry in &self.entries {
            entry.expand(context, &[], &mut |context, e| {
                let weight = e.get_weight(context.luck());
                if weight > 0 {
                    valid_entries.push(e);
                    total_weight += weight;
                }
            });
        }
        let entry_count = valid_entries.len();
        if total_weight != 0 && entry_count != 0 {
            if entry_count == 1 {
                valid_entries[0].create_item_stack(result, context);
            } else {
                let mut index = context.random().next_int_bounded(total_weight);
                for entry in &valid_entries {
                    index -= entry.get_weight(context.luck());
                    if index < 0 {
                        entry.create_item_stack(result, context);
                        return;
                    }
                }
            }
        }
    }

    pub fn add_random_items<'a>(&self, result: &mut dyn FnMut(&mut LootContext<'a>, ItemStack), context: &mut LootContext<'a>) {
        if self.condition.as_ref().is_none_or(|condition| condition.value().test(context)) {
            let mut decorated_consumer = |context: &mut LootContext<'a>, item_stack: ItemStack| loot_item_function::decorate(&self.modifier, item_stack, context, result);
            let count = self.rolls.value().get_int(context).wrapping_add(mth::floor_f(self.bonus_rolls.value().get_float(context) * context.luck()));
            for _ in 0..count {
                self.add_random_item(&mut decorated_consumer, context);
            }
        }
    }
}
