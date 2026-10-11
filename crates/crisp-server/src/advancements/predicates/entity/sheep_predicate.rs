use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct SheepPredicate {
    sheared: Option<bool>,
}

impl SheepPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<SheepPredicate> {
        codec::map(tag)?;
        Ok(SheepPredicate { sheared: codec::decode_optional(tag, "sheared", codec::bool_value)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.mob(entity).and_then(|mob| mob.sheep_sheared()).is_some_and(|sheared| self.sheared.is_none_or(|expected| expected == sheared))
    }
}
