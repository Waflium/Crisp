use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct FishingHookPredicate {
    in_open_water: Option<bool>,
}

impl FishingHookPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FishingHookPredicate> {
        codec::map(tag)?;
        Ok(FishingHookPredicate { in_open_water: codec::decode_optional(tag, "in_open_water", codec::bool_value)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let Some(in_open_water) = self.in_open_water else { return true };
        level.fishing_hook_open_water(entity).is_some_and(|open_water| open_water == in_open_water)
    }
}
