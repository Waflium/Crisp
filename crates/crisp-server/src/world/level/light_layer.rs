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

    pub fn get_pathfinding_cost_from_light_levels(&self, pos: BlockPos) -> f32 {
        self.get_light_level_dependent_magic_value(pos) - 0.5
    }

    pub fn get_light_level_dependent_magic_value(&self, pos: BlockPos) -> f32 {
        let v = self.max_local_raw_brightness(pos, self.sky_darken) as f32 / 15.0;
        let curved_v = v / (4.0 - 3.0 * v);
        crisp_core::mth::lerp_f(self.dimension_type().ambient_light, curved_v, 1.0)
    }

    pub fn max_local_raw_brightness(&self, pos: BlockPos, darkening: i32) -> i32 {
        if pos.x >= -30_000_000 && pos.z >= -30_000_000 && pos.x < 30_000_000 && pos.z < 30_000_000 { self.raw_brightness(pos, darkening) } else { 15 }
    }
}
