use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::min_max_bounds::Ints;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct CubeMobPredicate {
    size: Ints,
}

impl CubeMobPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<CubeMobPredicate> {
        codec::map(tag)?;
        Ok(CubeMobPredicate { size: codec::decode_or(tag, "size", Ints::ANY, Ints::decode)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.mob(entity).and_then(|mob| mob.cube_mob_size()).is_some_and(|size| self.size.matches(size))
    }
}
