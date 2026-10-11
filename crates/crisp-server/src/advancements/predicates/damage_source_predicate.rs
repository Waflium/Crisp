use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::entity::entity_predicate::EntityPredicate;
use super::tag_predicate::TagPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::damagesource::DamageSource;

#[derive(Debug)]
pub struct DamageSourcePredicate {
    tags: Vec<TagPredicate>,
    direct_entity: Option<EntityPredicate>,
    source_entity: Option<EntityPredicate>,
    is_direct: Option<bool>,
}

impl DamageSourcePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DamageSourcePredicate> {
        codec::map(tag)?;
        Ok(DamageSourcePredicate {
            tags: codec::decode_or(tag, "tags", Vec::new(), |t| codec::list_of(t, |e| TagPredicate::decode("minecraft:damage_type", e)))?,
            direct_entity: codec::decode_optional(tag, "direct_entity", EntityPredicate::decode)?,
            source_entity: codec::decode_optional(tag, "source_entity", EntityPredicate::decode)?,
            is_direct: codec::decode_optional(tag, "is_direct", codec::bool_value)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, position: Option<Vec3>, source: &DamageSource) -> bool {
        if !self.tags.iter().all(|tag| tag.matches(source.ty.0 as i32)) {
            return false;
        }
        if self.direct_entity.as_ref().is_some_and(|direct| !direct.matches(level, position, source.direct_entity)) {
            return false;
        }
        if self.source_entity.as_ref().is_some_and(|entity| !entity.matches(level, position, source.causing_entity)) {
            return false;
        }
        self.is_direct.is_none_or(|is_direct| is_direct == source.is_direct())
    }
}
