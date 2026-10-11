use crisp_protocol::nbt::Nbt;

use super::loot_item_function::LootItemFunction;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::container_component_manipulator::ContainerComponentManipulator;
use crate::world::level::storage::loot::container_component_manipulators;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct ModifyContainerContents {
    component: ContainerComponentManipulator,
    modifier: Holder<LootItemFunction>,
}

impl ModifyContainerContents {
    pub fn decode(tag: &Nbt) -> DataResult<ModifyContainerContents> {
        Ok(ModifyContainerContents {
            component: codec::decode_field(tag, "component", container_component_manipulators::decode)?,
            modifier: codec::decode_field(tag, "modifier", LootItemFunction::decode_holder)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        self.component.modify_items(&mut item_stack, |c| self.modifier.value().apply(c, context));
        item_stack
    }
}
