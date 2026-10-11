use crisp_protocol::nbt::Nbt;

use super::container_component_manipulator::ContainerComponentManipulator;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::DataComponentType;

pub const ALL_MANIPULATORS: [ContainerComponentManipulator; 3] =
    [ContainerComponentManipulator::Container, ContainerComponentManipulator::BundleContents, ContainerComponentManipulator::ChargedProjectiles];

pub fn decode(tag: &Nbt) -> DataResult<ContainerComponentManipulator> {
    let id = codec::identifier(tag)?;
    let ty = DataComponentType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"))?;
    ALL_MANIPULATORS.into_iter().find(|manipulator| manipulator.type_() == ty).ok_or_else(|| "No items in component".to_string())
}
