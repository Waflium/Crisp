use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::container_component_manipulator::ContainerComponentManipulator;
use crate::world::level::storage::loot::container_component_manipulators;
use crate::world::level::storage::loot::entries::loot_pool_entries;
use crate::world::level::storage::loot::entries::loot_pool_entry_container::LootPoolEntryContainer;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_table;

pub struct SetContainerContents {
    component: ContainerComponentManipulator,
    entries: Vec<LootPoolEntryContainer>,
}

impl SetContainerContents {
    pub fn decode(tag: &Nbt) -> DataResult<SetContainerContents> {
        Ok(SetContainerContents {
            component: codec::decode_field(tag, "component", container_component_manipulators::decode)?,
            entries: codec::decode_field(tag, "entries", |t| codec::list_of(t, loot_pool_entries::decode))?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        let mut contents = Vec::new();
        {
            let mut add = |_: &mut LootContext, item_stack: ItemStack| contents.push(item_stack);
            let mut splitter = loot_table::create_stack_splitter(&mut add);
            for e in &self.entries {
                e.expand(context, &[], &mut |context, entry| entry.create_item_stack(&mut splitter, context));
            }
        }
        self.component.set_contents(&mut item_stack, contents);
        item_stack
    }
}
