use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct RaiderPredicate {
    has_raid: bool,
    is_captain: bool,
}

impl RaiderPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<RaiderPredicate> {
        codec::map(tag)?;
        Ok(RaiderPredicate {
            has_raid: codec::decode_or(tag, "has_raid", false, codec::bool_value)?,
            is_captain: codec::decode_or(tag, "is_captain", false, codec::bool_value)?,
        })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.mob(entity).and_then(|mob| mob.raider()).is_some_and(|(has_raid, is_captain)| has_raid == self.has_raid && is_captain == self.is_captain)
    }
}
