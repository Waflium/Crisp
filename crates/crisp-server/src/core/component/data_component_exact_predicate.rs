use crisp_protocol::nbt::Nbt;

use super::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::DataComponentType;

#[derive(Clone, Debug, Default)]
pub struct DataComponentExactPredicate {
    expected_components: Vec<(DataComponentType, Nbt)>,
}

impl DataComponentExactPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DataComponentExactPredicate> {
        let expected_components = codec::map(tag)?
            .iter()
            .map(|(id, value)| {
                let id = crate::resources::identifier::read(id)?;
                let ty = DataComponentType::by_name(&id).ok_or_else(|| format!("No element named '{id}' in registry minecraft:data_component_type"))?;
                Ok((ty, ty.decode(value)?))
            })
            .collect::<DataResult<_>>()?;
        Ok(DataComponentExactPredicate { expected_components })
    }

    pub fn is_empty(&self) -> bool {
        self.expected_components.is_empty()
    }

    pub fn test(&self, actual_components: &(impl DataComponentGetter + ?Sized)) -> bool {
        self.expected_components.iter().all(|(ty, expected)| actual_components.get(*ty).is_some_and(|actual| *actual == *expected))
    }
}
