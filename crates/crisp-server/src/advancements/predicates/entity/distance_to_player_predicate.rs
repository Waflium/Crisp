use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::distance_predicate::DistancePredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct DistanceToPlayerPredicate {
    distance: DistancePredicate,
}

impl DistanceToPlayerPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DistanceToPlayerPredicate> {
        Ok(DistanceToPlayerPredicate { distance: DistancePredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let Some(position) = position else { return false };
        let e = level.entity(entity);
        self.distance.matches(position.x, position.y, position.z, e.x(), e.y(), e.z())
    }
}
