use crisp_protocol::nbt::Nbt;

use super::ContextIntProvider;
use crate::core::holder::RegistryElement;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::DataComponentType;
use crate::world::level::storage::loot::loot_context::LootContext;

pub enum ResolvableInt {
    Constant(i32),
    Reference(String),
}

impl ResolvableInt {
    pub fn decode(tag: &Nbt) -> DataResult<ResolvableInt> {
        match codec::either(tag, codec::int, codec::identifier)? {
            Ok(value) => Ok(ResolvableInt::Constant(value)),
            Err(key) => Ok(ResolvableInt::Reference(key)),
        }
    }

    pub fn get_from_item(
        item_stack: &ItemStack,
        component_type: DataComponentType,
        getter: impl Fn(&Nbt) -> Option<&Nbt>,
        context: &mut LootContext,
        default_value: i32,
    ) -> i32 {
        match item_stack.get(component_type).and_then(getter).map(ResolvableInt::decode) {
            Some(Ok(resolvable)) => resolvable.get(context, default_value),
            _ => default_value,
        }
    }

    pub fn get(&self, context: &mut LootContext, default_value: i32) -> i32 {
        match self {
            ResolvableInt::Constant(value) => *value,
            ResolvableInt::Reference(key) => ContextIntProvider::lookup(key).map_or(default_value, |provider| provider.get_int(context)),
        }
    }
}
