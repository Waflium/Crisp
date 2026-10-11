use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::entity_predicate::EntityPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct TargetedEntityPredicate {
    targeted_entity: Box<EntityPredicate>,
}

impl TargetedEntityPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<TargetedEntityPredicate> {
        Ok(TargetedEntityPredicate { targeted_entity: Box::new(EntityPredicate::decode(tag)?) })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let target = level.mob(entity).and_then(|mob| mob.target);
        self.targeted_entity.matches(level, position, target)
    }
}
