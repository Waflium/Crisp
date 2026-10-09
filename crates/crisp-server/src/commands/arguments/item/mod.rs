pub mod component_predicate_parser;
pub mod item_parser;
pub mod item_predicate_argument;

use crisp_data::Item;

use crate::commands::brigadier::{CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, tr};
use crate::world::item::ItemStack;
use crate::world::item::component::DataComponentPatch;

pub static ERROR_STACK_TOO_BIG: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|item, count| tr("arguments.item.overstacked", vec![item, count]));
pub static ERROR_MALFORMED_ITEM: DynamicCommandExceptionType = DynamicCommandExceptionType(|error| tr("arguments.item.malformed", vec![error]));

#[derive(Clone, Debug, PartialEq)]
pub struct ItemInput {
    pub item: Item,
    pub components: DataComponentPatch,
}

impl ItemInput {
    pub fn create_item_stack(&self, count: i32) -> Result<ItemStack, CommandSyntaxException> {
        let result = ItemStack::with_patch(self.item, count, &self.components);
        if count > result.max_stack_size() {
            return Err(ERROR_STACK_TOO_BIG.create(self.item.name(), result.max_stack_size()));
        }
        result.validate_strict().map_err(|e| ERROR_MALFORMED_ITEM.create(e))?;
        Ok(result)
    }
}
