use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::clock::ServerClockManager;
use crate::world::level::storage::loot::int_range_predicate::IntRangePredicate;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct TimeCheck {
    clock: usize,
    period: Option<i64>,
    value: IntRangePredicate,
}

impl TimeCheck {
    pub fn decode(tag: &Nbt) -> DataResult<TimeCheck> {
        let clock = codec::decode_field(tag, "clock", |t| {
            let id = codec::identifier(t)?;
            ServerClockManager::id_of(&id).ok_or_else(|| format!("Missing element ResourceKey[minecraft:world_clock / {id}]"))
        })?;
        Ok(TimeCheck { clock, period: codec::decode_optional(tag, "period", codec::long)?, value: codec::decode_field(tag, "value", IntRangePredicate::decode)? })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let mut time = context.params.level.clock_ticks[self.clock];
        if let Some(period) = self.period {
            time = time.wrapping_rem(period);
        }
        self.value.test(context, time as i32)
    }
}
