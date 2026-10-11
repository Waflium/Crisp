use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;

pub const NO_LIMIT: i32 = 0;

pub struct EnchantedCountIncreaseFunction {
    enchantment: String,
    count: Holder<ContextFloatProvider>,
    limit: i32,
}

impl EnchantedCountIncreaseFunction {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantedCountIncreaseFunction> {
        Ok(EnchantedCountIncreaseFunction {
            enchantment: codec::decode_field(tag, "enchantment", Enchantment::decode_holder)?,
            count: codec::decode_field(tag, "count", ContextFloatProvider::decode_holder)?,
            limit: codec::decode_or(tag, "limit", NO_LIMIT, codec::int)?,
        })
    }

    fn has_limit(&self) -> bool {
        self.limit > 0
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let Some(killer) = context.params.attacking_entity.filter(|&e| context.params.level.is_living(e)) else { return item_stack };
        let level = context.params.level.get_enchantment_level(&self.enchantment, killer);
        if level == 0 {
            return item_stack;
        }
        let addition = level as f32 * self.count.value().get_float(context);
        item_stack.grow(crate::util::java_round_f(addition));
        if self.has_limit() {
            item_stack.limit_size(self.limit);
        }
        item_stack
    }
}
