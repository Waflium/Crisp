use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::location_predicate::LocationPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityLocationPredicate {
    predicate: LocationPredicate,
}

impl EntityLocationPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityLocationPredicate> {
        Ok(EntityLocationPredicate { predicate: LocationPredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId) -> bool {
        let e = level.entity(entity);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.predicate.matches(level, x, y, z)
    }
}
