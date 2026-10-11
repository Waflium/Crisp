use crisp_core::BlockPos;
use crisp_data::{BlockState, Fluid};

use super::FlowingFluid;
use crate::server::level::ServerLevel;
use crate::world::level::LevelReader;

const WATER_SOURCE_CONVERSION: bool = true;

pub struct WaterFluid;

impl FlowingFluid for WaterFluid {
    fn flowing(&self) -> Fluid {
        Fluid::FlowingWater
    }

    fn source(&self) -> Fluid {
        Fluid::Water
    }

    fn can_convert_to_source(&self, _level: &ServerLevel) -> bool {
        WATER_SOURCE_CONVERSION
    }

    fn before_destroying_block(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        level.drop_resources(state, pos, None, None);
    }

    fn slope_find_distance(&self, _level: &ServerLevel) -> i32 {
        4
    }

    fn drop_off(&self, _level: &ServerLevel) -> i32 {
        1
    }

    fn tick_delay(&self, _level: &dyn LevelReader) -> i32 {
        5
    }
}
