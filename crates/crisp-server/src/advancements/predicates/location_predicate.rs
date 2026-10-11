use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::block_predicate::BlockPredicate;
use super::fluid_predicate::FluidPredicate;
use super::light_predicate::LightPredicate;
use super::min_max_bounds::Doubles;
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::level::block::campfire_block;

#[derive(Debug)]
struct PositionPredicate {
    x: Doubles,
    y: Doubles,
    z: Doubles,
}

impl PositionPredicate {
    fn decode(tag: &Nbt) -> DataResult<PositionPredicate> {
        codec::map(tag)?;
        Ok(PositionPredicate {
            x: codec::decode_or(tag, "x", Doubles::ANY, Doubles::decode)?,
            y: codec::decode_or(tag, "y", Doubles::ANY, Doubles::decode)?,
            z: codec::decode_or(tag, "z", Doubles::ANY, Doubles::decode)?,
        })
    }

    fn matches(&self, x: f64, y: f64, z: f64) -> bool {
        self.x.matches(x) && self.y.matches(y) && self.z.matches(z)
    }
}

#[derive(Debug)]
pub struct LocationPredicate {
    position: Option<PositionPredicate>,
    biomes: Option<HolderSet>,
    structures: Option<Vec<String>>,
    dimension: Option<String>,
    smokey: Option<bool>,
    light: Option<LightPredicate>,
    block: Option<BlockPredicate>,
    fluid: Option<FluidPredicate>,
    can_see_sky: Option<bool>,
}

impl LocationPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<LocationPredicate> {
        codec::map(tag)?;
        Ok(LocationPredicate {
            position: codec::decode_optional(tag, "position", PositionPredicate::decode)?,
            biomes: codec::decode_optional(tag, "biomes", |t| HolderSet::decode("minecraft:worldgen/biome", t))?,
            structures: codec::decode_optional(tag, "structures", |t| codec::compact_list_of(t, |s| Ok(codec::string(s)?.to_string())))?,
            dimension: codec::decode_optional(tag, "dimension", codec::identifier)?,
            smokey: codec::decode_optional(tag, "smokey", codec::bool_value)?,
            light: codec::decode_optional(tag, "light", LightPredicate::decode)?,
            block: codec::decode_optional(tag, "block", BlockPredicate::decode)?,
            fluid: codec::decode_optional(tag, "fluid", FluidPredicate::decode)?,
            can_see_sky: codec::decode_optional(tag, "can_see_sky", codec::bool_value)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, x: f64, y: f64, z: f64) -> bool {
        if self.position.as_ref().is_some_and(|position| !position.matches(x, y, z)) {
            return false;
        }
        if self.dimension.as_ref().is_some_and(|dimension| dimension != crate::server::OVERWORLD) {
            return false;
        }
        let pos = BlockPos::containing(x, y, z);
        let loaded = level.is_loaded(pos);
        if self.biomes.as_ref().is_some_and(|biomes| !(loaded && biomes.contains(level.get_biome(pos).0 as i32))) {
            return false;
        }
        if self.structures.is_some() {
            return false;
        }
        if self.smokey.is_some_and(|smokey| !(loaded && smokey == campfire_block::is_smokey_pos(level, pos))) {
            return false;
        }
        if self.light.as_ref().is_some_and(|light| !light.matches(level, pos)) {
            return false;
        }
        if self.block.as_ref().is_some_and(|block| !block.matches(level, pos)) {
            return false;
        }
        if self.fluid.as_ref().is_some_and(|fluid| !fluid.matches(level, pos)) {
            return false;
        }
        self.can_see_sky.is_none_or(|can_see_sky| can_see_sky == level.can_see_sky(pos))
    }
}
