use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::entity_predicate::EntityPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::{EntityId, LevelEntity};

#[derive(Debug)]
pub struct LightningBoltPredicate {
    blocks_set_on_fire: Ints,
    entity_struck: Option<Box<EntityPredicate>>,
}

impl LightningBoltPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<LightningBoltPredicate> {
        codec::map(tag)?;
        Ok(LightningBoltPredicate {
            blocks_set_on_fire: codec::decode_or(tag, "blocks_set_on_fire", Ints::ANY, Ints::decode)?,
            entity_struck: codec::decode_optional(tag, "entity_struck", |t| Ok(Box::new(EntityPredicate::decode(t)?)))?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let Some(LevelEntity::LightningBolt(bolt)) = level.entities.get(&entity) else { return false };
        if !self.blocks_set_on_fire.matches(bolt.blocks_set_on_fire) {
            return false;
        }
        let Some(entity_struck) = &self.entity_struck else { return true };
        let hit: Vec<EntityId> = bolt.hit_entities().collect();
        hit.into_iter().any(|e| entity_struck.matches(level, position, Some(e)))
    }
}
