use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};

#[derive(Clone, Copy)]
enum Target {
    CustomName,
    ItemName,
}

impl Target {
    fn component(self) -> DataComponentType {
        match self {
            Target::CustomName => dc::CUSTOM_NAME,
            Target::ItemName => dc::ITEM_NAME,
        }
    }
}

pub fn decode_component(tag: &Nbt) -> DataResult<Nbt> {
    dc::CUSTOM_NAME.decode(tag)
}

pub struct SetNameFunction {
    name: Option<Nbt>,
    resolution_context: Option<EntityTarget>,
    target: Target,
}

impl SetNameFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetNameFunction> {
        Ok(SetNameFunction {
            name: codec::decode_optional(tag, "name", decode_component)?,
            resolution_context: codec::decode_optional(tag, "entity", EntityTarget::decode)?,
            target: codec::decode_or(tag, "target", Target::CustomName, |t| codec::string_enum(t, &[("custom_name", Target::CustomName), ("item_name", Target::ItemName)]))?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if let Some(name) = &self.name {
            let _ = self.resolution_context.and_then(|target| target.get(&context.params));
            item_stack.set(self.target.component(), Some(name.clone()));
        }
        item_stack
    }
}
