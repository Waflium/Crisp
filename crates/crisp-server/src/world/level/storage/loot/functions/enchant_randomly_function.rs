use crisp_core::mth;
use crisp_core::random::RandomSource;
use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::codec::registry;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::enchant;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct EnchantRandomlyFunction {
    options: Option<HolderSet>,
    only_compatible: bool,
    include_additional_cost_component: bool,
}

impl EnchantRandomlyFunction {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantRandomlyFunction> {
        Ok(EnchantRandomlyFunction {
            options: codec::decode_optional(tag, "options", |t| HolderSet::decode("minecraft:enchantment", t))?,
            only_compatible: codec::decode_or(tag, "only_compatible", true, codec::bool_value)?,
            include_additional_cost_component: codec::decode_or(tag, "include_additional_cost_component", false, codec::bool_value)?,
        })
    }

    pub fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let target_is_book = item_stack.is(items::BOOK);
        let should_check_compatibility = !target_is_book && self.only_compatible;
        let names = registry("minecraft:enchantment");
        let candidates: Vec<&'static str> = match &self.options {
            Some(options) => options.contents().iter().map(|&id| names[id as usize]).collect(),
            None => names.to_vec(),
        };
        let compatible_enchantments: Vec<&'static str> = candidates
            .into_iter()
            .filter(|candidate| !should_check_compatibility || Enchantment::by_id(candidate).is_some_and(|e| e.can_enchant(&item_stack)))
            .collect();
        if compatible_enchantments.is_empty() {
            warn!("Couldn't find a compatible enchantment for {} {}", item_stack.count, item_stack.item.name());
            return item_stack;
        }
        let enchantment = compatible_enchantments[context.random().next_int_bounded(compatible_enchantments.len() as i32) as usize];
        self.enchant_item(item_stack, enchantment, context)
    }

    fn enchant_item(&self, mut item_stack: ItemStack, enchantment: &str, context: &mut LootContext) -> ItemStack {
        let definition = Enchantment::by_id(enchantment).expect("a registered enchantment");
        let level = mth::next_int(&mut context.random(),definition.min_level(), definition.max_level());
        if item_stack.is(items::BOOK) {
            item_stack = ItemStack::new(items::ENCHANTED_BOOK, 1);
        }
        enchant(&mut item_stack, enchantment, level);
        if self.include_additional_cost_component && context.has_parameter(LootContextParam::AdditionalCostComponentAllowed) {
            let cost = 2 + context.random().next_int_bounded(5 + level * 10) + 3 * level;
            item_stack.set(dc::ADDITIONAL_TRADE_COST, Some(Nbt::Int(cost)));
        }
        item_stack
    }
}
