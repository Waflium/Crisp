use crisp_protocol::nbt::Nbt;

use super::ContextFloatProvider;
use crate::core::holder::RegistryElement;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::DataComponentType;
use crate::world::level::storage::loot::loot_context::LootContext;

pub enum ResolvableFloat {
    Constant(f32),
    Reference(String),
}

impl ResolvableFloat {
    pub fn decode(tag: &Nbt) -> DataResult<ResolvableFloat> {
        match codec::either(tag, codec::float, codec::identifier)? {
            Ok(value) => Ok(ResolvableFloat::Constant(value)),
            Err(key) => Ok(ResolvableFloat::Reference(key)),
        }
    }

    pub fn get_from_item(
        item_stack: &ItemStack,
        component_type: DataComponentType,
        getter: impl Fn(&Nbt) -> Option<&Nbt>,
        context: &mut LootContext,
        default_value: f32,
    ) -> f32 {
        match item_stack.get(component_type).and_then(getter).map(ResolvableFloat::decode) {
            Some(Ok(resolvable)) => resolvable.get(context, default_value),
            _ => default_value,
        }
    }

    pub fn get(&self, context: &mut LootContext, default_value: f32) -> f32 {
        match self {
            ResolvableFloat::Constant(value) => *value,
            ResolvableFloat::Reference(key) => ContextFloatProvider::lookup(key).map_or(default_value, |provider| provider.get_float(context)),
        }
    }
}
