use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::crafting::{CookingRecipeType, cooking_recipes, get_recipe_for};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SmeltItemFunction {
    use_input_count: bool,
}

impl SmeltItemFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SmeltItemFunction> {
        Ok(SmeltItemFunction { use_input_count: codec::decode_or(tag, "use_input_count", true, codec::bool_value)? })
    }

    pub fn run(&self, item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        if let Some(recipe) = get_recipe_for(CookingRecipeType::Smelting, &item_stack, None) {
            let result = cooking_recipes()[recipe].assemble();
            if !result.is_empty() {
                let new_count = if self.use_input_count { item_stack.count } else { 1 } * result.count;
                return result.copy_with_count(new_count.min(result.max_stack_size()));
            }
        }
        warn!("Couldn't smelt {} {} because there is no smelting recipe", item_stack.count, item_stack.item.name());
        item_stack
    }
}
