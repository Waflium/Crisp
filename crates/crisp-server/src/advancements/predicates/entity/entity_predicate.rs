use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::entity_sub_predicate::EntitySubPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::LootParams;

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

    pub fn matches_player(&self, level: &mut ServerLevel, player: EntityId, entity: Option<EntityId>) -> bool {
        let position = level.entity(player).position();
        self.matches(level, Some(position), entity)
    }

    pub fn create_context(level: &mut ServerLevel, player: EntityId, entity: EntityId) -> LootContext<'_> {
        let origin = level.entity(player).position();
        let mut params = LootParams::new(level);
        params.this_entity = Some(entity);
        params.origin = Some(origin);
        LootContextBuilder::new(params).create(None)
    }
}
