use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::entity_predicate::EntityPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct VehiclePredicate {
    vehicle: Box<EntityPredicate>,
}

impl VehiclePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<VehiclePredicate> {
        Ok(VehiclePredicate { vehicle: Box::new(EntityPredicate::decode(tag)?) })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let vehicle = level.vehicle(entity);
        self.vehicle.matches(level, position, vehicle)
    }
}
