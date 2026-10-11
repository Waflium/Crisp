use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::location_predicate::LocationPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct MovementAffectedByPredicate {
    predicate: LocationPredicate,
}

impl MovementAffectedByPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<MovementAffectedByPredicate> {
        Ok(MovementAffectedByPredicate { predicate: LocationPredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId) -> bool {
        let on_pos = Vec3::at_center_of(level.block_pos_below_that_affects_my_movement(entity));
        self.predicate.matches(level, on_pos.x, on_pos.y, on_pos.z)
    }
}
