use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::min_max_bounds::Ints;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub struct LightPredicate {
    composite: Ints,
}

impl LightPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<LightPredicate> {
        codec::map(tag)?;
        Ok(LightPredicate { composite: codec::decode_or(tag, "light", Ints::ANY, Ints::decode)? })
    }

    pub fn matches(&self, level: &ServerLevel, pos: BlockPos) -> bool {
        level.is_loaded(pos) && self.composite.matches(level.max_local_raw_brightness(pos, level.sky_darken()))
    }
}
