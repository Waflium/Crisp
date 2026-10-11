use crisp_core::mth;
use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::update_enchantments;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub struct SetEnchantmentsFunction {
    enchantments: Vec<(String, Holder<ContextIntProvider>)>,
    add: bool,
}

impl SetEnchantmentsFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetEnchantmentsFunction> {
        Ok(SetEnchantmentsFunction {
            enchantments: codec::decode_or(tag, "enchantments", Vec::new(), |t| {
                codec::unbounded_map(t, |k| Enchantment::decode_holder(&Nbt::String(k.to_string())), ContextIntProvider::decode_holder)
            })?,
            add: codec::decode_or(tag, "add", false, codec::bool_value)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is(items::BOOK) {
            item_stack = item_stack.transmute_copy(items::ENCHANTED_BOOK, item_stack.count);
        }
        let mut levels = Vec::with_capacity(self.enchantments.len());
        update_enchantments(&mut item_stack, |enchantments| {
            for (enchantment, level_provider) in &self.enchantments {
                let level = level_provider.value().get_int(context);
                let level = if self.add { enchantments.level(enchantment).wrapping_add(level) } else { level };
                levels.push((enchantment, mth::clamp_i(level, 0, 255)));
            }
            for (enchantment, level) in levels.drain(..) {
                enchantments.set(enchantment, level);
            }
        });
        item_stack
    }
}
