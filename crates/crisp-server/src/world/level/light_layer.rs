use crisp_core::BlockPos;

use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LightLayer {
    Sky,
    Block,
}

impl ServerLevel {
    pub fn brightness(&self, layer: LightLayer, pos: BlockPos) -> i32 {
        self.chunks.light_engine.engine.light_value(layer, pos)
    }

    pub fn raw_brightness(&self, pos: BlockPos, darkening: i32) -> i32 {
        self.chunks.light_engine.engine.raw_brightness(pos, darkening)
    }

    pub fn sky_darken(&self) -> i32 {
        self.sky_darken
    }

    pub fn update_sky_brightness(&mut self) {
        self.sky_darken = (15.0 - self.environment_attributes().get_dimension_value(environment_attributes::SKY_LIGHT_LEVEL)) as i32;
    }

    pub fn is_bright_outside(&self) -> bool {
        !self.dimension_type().has_fixed_time && self.sky_darken < 4
    }

    pub fn is_dark_outside(&self) -> bool {
        !self.dimension_type().has_fixed_time && !self.is_bright_outside()
    }

    pub fn max_local_raw_brightness(&self, pos: BlockPos, darkening: i32) -> i32 {
        if pos.x >= -30_000_000 && pos.z >= -30_000_000 && pos.x < 30_000_000 && pos.z < 30_000_000 { self.raw_brightness(pos, darkening) } else { 15 }
    }
}
