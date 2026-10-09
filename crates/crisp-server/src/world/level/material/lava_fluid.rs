use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::{BlockState, Fluid, FluidState, block_tags, blocks, fluid_tags};

use super::{FlowingFluid, height};
use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::server::level::events::LAVA_FIZZ;
use crate::server::level::game_mode::GameType;
use crate::world::level::BlockGetter;
use crate::world::level::block::base_fire_block;
use crate::world::level::update_flags::UPDATE_ALL;

const LAVA_SOURCE_CONVERSION: bool = false;
const FIRE_SPREAD_RADIUS_AROUND_PLAYER: i32 = 128;

pub struct LavaFluid;

fn is_fast_lava(level: &ServerLevel) -> bool {
    level.environment_attributes().get_dimension_value(environment_attributes::FAST_LAVA)
}

fn fizz(level: &mut ServerLevel, pos: BlockPos) {
    level.level_event(None, LAVA_FIZZ, pos, 0);
}

impl LavaFluid {
    fn has_flammable_neighbours(level: &mut ServerLevel, pos: BlockPos) -> bool {
        crisp_core::Direction::VALUES.into_iter().any(|direction| Self::is_flammable(level, pos.relative(direction)))
    }

    fn is_flammable(level: &mut ServerLevel, pos: BlockPos) -> bool {
        if !level.chunks.is_outside_build_height(pos.y) && !level.chunks.has_chunk(pos.chunk()) {
            return false;
        }
        level.block_state(pos).ignited_by_lava()
    }
}

impl FlowingFluid for LavaFluid {
    fn flowing(&self) -> Fluid {
        Fluid::FlowingLava
    }

    fn source(&self) -> Fluid {
        Fluid::Lava
    }

    fn can_convert_to_source(&self, _level: &ServerLevel) -> bool {
        LAVA_SOURCE_CONVERSION
    }

    fn before_destroying_block(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        fizz(level, pos);
    }

    fn slope_find_distance(&self, level: &ServerLevel) -> i32 {
        if is_fast_lava(level) { 4 } else { 2 }
    }

    fn drop_off(&self, level: &ServerLevel) -> i32 {
        if is_fast_lava(level) { 1 } else { 2 }
    }

    fn tick_delay(&self, level: &ServerLevel) -> i32 {
        if is_fast_lava(level) { 10 } else { 30 }
    }

    fn spread_delay(&self, level: &mut ServerLevel, pos: BlockPos, old: FluidState, new: FluidState) -> i32 {
        let mut result = self.tick_delay(level);
        if !old.is_empty()
            && !new.is_empty()
            && !old.falling
            && !new.falling
            && height(level, new, pos) > height(level, old, pos)
            && level.random.next_int_bounded(4) != 0
        {
            result *= 4;
        }
        result
    }

    fn spread_to(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, direction: crisp_core::Direction, target: FluidState) {
        if direction == crisp_core::Direction::Down {
            let fluid = level.fluid_state(pos);
            if fluid.is(&fluid_tags::WATER) {
                if state.block().is_instance_of("LiquidBlock") {
                    level.set_block(pos, blocks::STONE.default_state(), UPDATE_ALL);
                }
                fizz(level, pos);
                return;
            }
        }
        self.flowing_spread_to(level, pos, state, direction, target);
    }

    fn random_tick(&self, level: &mut ServerLevel, pos: BlockPos, _fluid: FluidState) {
        if !level.can_spread_fire_around(pos) {
            return;
        }
        let passes = level.random.next_int_bounded(3);
        if passes > 0 {
            let mut test_pos = pos;
            for _ in 0..passes {
                let dx = level.random.next_int_bounded(3) - 1;
                let dz = level.random.next_int_bounded(3) - 1;
                test_pos = test_pos.offset(dx, 1, dz);
                if !level.is_loaded(test_pos) {
                    return;
                }
                let block_state = level.block_state(test_pos);
                if block_state.is_air() {
                    if Self::has_flammable_neighbours(level, test_pos) {
                        let fire = base_fire_block::get_state(level, test_pos);
                        level.set_block(test_pos, fire, UPDATE_ALL);
                        return;
                    }
                } else if block_state.is(&block_tags::BLOCKS_LAVA_FIRE_SPREAD) {
                    return;
                }
            }
        } else {
            for _ in 0..3 {
                let dx = level.random.next_int_bounded(3) - 1;
                let dz = level.random.next_int_bounded(3) - 1;
                let test_pos = pos.offset(dx, 0, dz);
                if !level.is_loaded(test_pos) {
                    return;
                }
                if level.block_state(test_pos.above()).is_air() && Self::is_flammable(level, test_pos) {
                    let fire = base_fire_block::get_state(level, test_pos);
                    level.set_block(test_pos.above(), fire, UPDATE_ALL);
                }
            }
        }
    }
}

impl ServerLevel {
    pub fn can_spread_fire_around(&self, pos: BlockPos) -> bool {
        let radius = FIRE_SPREAD_RADIUS_AROUND_PLAYER;
        let target = Vec3::at_lower_corner_of(pos);
        radius == -1
            || self.players.values().any(|p| p.game_mode.game_type != GameType::Spectator && p.entity.position().distance_to(target) < radius as f64)
    }

    pub fn is_loaded(&self, pos: BlockPos) -> bool {
        self.is_in_valid_bounds(pos) && self.chunks.has_chunk(pos.chunk())
    }
}
