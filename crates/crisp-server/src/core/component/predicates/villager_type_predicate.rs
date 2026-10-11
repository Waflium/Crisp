use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::DataResult;
use crate::world::item::component::codec::registry_id;

#[derive(Debug)]
pub struct VillagerTypePredicate {
    villager_types: HolderSet,
}

impl VillagerTypePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<VillagerTypePredicate> {
        Ok(VillagerTypePredicate { villager_types: HolderSet::decode("minecraft:villager_type", tag)? })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        value.as_str().and_then(|id| registry_id("minecraft:villager_type", id).ok()).is_some_and(|id| self.villager_types.contains(id))
    }
}
