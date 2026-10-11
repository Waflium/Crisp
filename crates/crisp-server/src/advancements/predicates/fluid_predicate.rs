use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::state_properties_predicate::StatePropertiesPredicate;
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::level::BlockGetter;

#[derive(Debug)]
pub struct FluidPredicate {
    fluids: Option<HolderSet>,
    properties: Option<StatePropertiesPredicate>,
}

impl FluidPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FluidPredicate> {
        codec::map(tag)?;
        Ok(FluidPredicate {
            fluids: codec::decode_optional(tag, "fluids", |t| HolderSet::decode("minecraft:fluid", t))?,
            properties: codec::decode_optional(tag, "state", StatePropertiesPredicate::decode)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        if !level.is_loaded(pos) {
            return false;
        }
        let state = level.fluid_state(pos);
        if self.fluids.as_ref().is_some_and(|fluids| !fluids.contains(state.fluid as i32)) {
            return false;
        }
        self.properties.as_ref().is_none_or(|properties| properties.matches_fluid(state))
    }
}
