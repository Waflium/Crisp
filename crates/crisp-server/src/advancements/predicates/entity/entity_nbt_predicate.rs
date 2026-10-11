use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::nbt_predicate::NbtPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityNbtPredicate {
    nbt: NbtPredicate,
}

impl EntityNbtPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityNbtPredicate> {
        Ok(EntityNbtPredicate { nbt: NbtPredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.nbt.matches(Some(&level.entity_tag_to_compare(entity)))
    }
}
