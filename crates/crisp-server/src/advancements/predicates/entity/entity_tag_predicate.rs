use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityTagPredicate {
    any_of: Option<Vec<String>>,
    all_of: Option<Vec<String>>,
    none_of: Option<Vec<String>>,
}

impl EntityTagPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityTagPredicate> {
        codec::map(tag)?;
        let strings = |key| codec::decode_optional(tag, key, |t| codec::list_of(t, |s| Ok(codec::string(s)?.to_string())));
        Ok(EntityTagPredicate { any_of: strings("any_of")?, all_of: strings("all_of")?, none_of: strings("none_of")? })
    }

    pub fn matches_tags(&self, tags: &[String]) -> bool {
        let contains_at_least_one = |list: &[String]| list.iter().any(|tag| tags.contains(tag));
        if self.any_of.as_ref().is_some_and(|any_of| !contains_at_least_one(any_of)) {
            return false;
        }
        if self.none_of.as_ref().is_some_and(|none_of| contains_at_least_one(none_of)) {
            return false;
        }
        self.all_of.as_ref().is_none_or(|all_of| all_of.iter().all(|tag| tags.contains(tag)))
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.matches_tags(&level.entity(entity).tags)
    }
}
