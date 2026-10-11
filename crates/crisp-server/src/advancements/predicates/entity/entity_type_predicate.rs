use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityTypePredicate {
    types: HolderSet,
}

impl EntityTypePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityTypePredicate> {
        Ok(EntityTypePredicate { types: HolderSet::decode("minecraft:entity_type", tag)? })
    }

    pub fn matches_type(&self, entity_type: i32) -> bool {
        self.types.contains(entity_type)
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.matches_type(level.entity(entity).entity_type)
    }
}
