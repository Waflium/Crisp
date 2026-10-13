use crisp_core::Vec3;

use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EntityHitResult {
    pub entity: EntityId,
    pub location: Vec3,
}

impl EntityHitResult {
    pub fn new(entity: EntityId, location: Vec3) -> EntityHitResult {
        EntityHitResult { entity, location }
    }
}
