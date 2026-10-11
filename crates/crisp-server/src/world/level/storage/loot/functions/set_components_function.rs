use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::DataComponentPatch;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetComponentsFunction {
    components: DataComponentPatch,
}

impl SetComponentsFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetComponentsFunction> {
        Ok(SetComponentsFunction { components: codec::decode_field(tag, "components", DataComponentPatch::from_nbt)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let _ = item_stack.apply_components_and_validate(&self.components);
        item_stack
    }
}
