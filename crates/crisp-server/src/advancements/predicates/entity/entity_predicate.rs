use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::entity_sub_predicate::EntitySubPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityPredicate {
    combined_part: Vec<EntitySubPredicate>,
}

impl EntityPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityPredicate> {
        let mut parts = codec::map(tag)?
            .iter()
            .map(|(key, value)| EntitySubPredicate::decode(&crate::resources::identifier::read(key)?, value))
            .collect::<DataResult<Vec<_>>>()?;
        if parts.len() > 1 {
            parts.sort_by_key(EntitySubPredicate::predicate_type_order);
        }
        Ok(EntityPredicate { combined_part: parts })
    }

    pub fn matches(&self, level: &mut ServerLevel, position: Option<Vec3>, entity: Option<EntityId>) -> bool {
        let Some(entity) = entity else { return false };
        self.combined_part.iter().all(|part| part.matches(level, entity, position))
    }
}
