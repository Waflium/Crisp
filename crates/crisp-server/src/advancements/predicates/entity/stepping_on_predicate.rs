use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::location_predicate::LocationPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct SteppingOnPredicate {
    predicate: LocationPredicate,
}

impl SteppingOnPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<SteppingOnPredicate> {
        Ok(SteppingOnPredicate { predicate: LocationPredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId) -> bool {
        if !level.entity(entity).on_ground {
            return false;
        }
        let on_pos = Vec3::at_center_of(level.on_pos(entity, 1.0e-5));
        self.predicate.matches(level, on_pos.x, on_pos.y, on_pos.z)
    }
}
