use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::codec::registry;
use crate::world::item::enchantment::enchantment_helper::enchant_item;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub struct EnchantWithLevelsFunction {
    levels: Holder<ContextIntProvider>,
    options: Option<HolderSet>,
    include_additional_cost_component: bool,
}

impl EnchantWithLevelsFunction {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantWithLevelsFunction> {
        Ok(EnchantWithLevelsFunction {
            levels: codec::decode_field(tag, "levels", ContextIntProvider::decode_holder)?,
            options: codec::decode_optional(tag, "options", |t| HolderSet::decode("minecraft:enchantment", t))?,
            include_additional_cost_component: codec::decode_or(tag, "include_additional_cost_component", false, codec::bool_value)?,
        })
    }

    pub fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let enchantment_cost = self.levels.value().get_int(context);
        let names = registry("minecraft:enchantment");
        let source: Vec<&str> = match &self.options {
            Some(options) => options.contents().iter().map(|&id| names[id as usize]).collect(),
            None => names.to_vec(),
        };
        let mut result = enchant_item(&mut context.random(),item_stack, enchantment_cost, &source);
        if self.include_additional_cost_component && context.has_parameter(LootContextParam::AdditionalCostComponentAllowed) && !result.is_empty() && enchantment_cost > 0 {
            result.set(dc::ADDITIONAL_TRADE_COST, Some(Nbt::Int(enchantment_cost)));
        }
        result
    }
}
