use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use super::set_potion_function::with_potion;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::codec::registry;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetRandomPotionFunction {
    options: Option<HolderSet>,
}

impl SetRandomPotionFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetRandomPotionFunction> {
        Ok(SetRandomPotionFunction { options: codec::decode_optional(tag, "options", |t| HolderSet::decode("minecraft:potion", t))? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let potions = registry("minecraft:potion");
        let potion = match &self.options {
            Some(options) => {
                let contents = options.contents();
                if contents.is_empty() { None } else { Some(contents[context.random().next_int_bounded(contents.len() as i32) as usize]) }
            }
            None => {
                if potions.is_empty() { None } else { Some(context.random().next_int_bounded(potions.len() as i32)) }
            }
        };
        if let Some(potion) = potion {
            with_potion(&mut item_stack, potions[potion as usize]);
        }
        item_stack
    }
}
