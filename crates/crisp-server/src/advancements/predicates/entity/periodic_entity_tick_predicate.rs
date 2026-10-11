use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct PeriodicEntityTickPredicate {
    periodic_tick: i32,
}

impl PeriodicEntityTickPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<PeriodicEntityTickPredicate> {
        let periodic_tick = codec::int(tag)?;
        if periodic_tick <= 0 {
            return Err(format!("Value must be positive: {periodic_tick}"));
        }
        Ok(PeriodicEntityTickPredicate { periodic_tick })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.entity(entity).tick_count % self.periodic_tick == 0
    }
}
