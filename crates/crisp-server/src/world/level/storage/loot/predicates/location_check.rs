use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::location_predicate::LocationPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct LocationCheck {
    predicate: Option<LocationPredicate>,
    offset: (i32, i32, i32),
}

impl LocationCheck {
    pub fn decode(tag: &Nbt) -> DataResult<LocationCheck> {
        let offset = |key| codec::decode_or(tag, key, 0, codec::int);
        Ok(LocationCheck {
            predicate: codec::decode_optional(tag, "predicate", LocationPredicate::decode)?,
            offset: (offset("offsetX")?, offset("offsetY")?, offset("offsetZ")?),
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(pos) = context.params.origin else { return false };
        let Some(predicate) = &self.predicate else { return true };
        let (x, y, z) = self.offset;
        predicate.matches(context.level(), pos.x + x as f64, pos.y + y as f64, pos.z + z as f64)
    }
}
