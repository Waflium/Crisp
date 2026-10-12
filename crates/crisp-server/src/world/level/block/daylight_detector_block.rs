use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::properties::{INVERTED, POWER};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::attribute::environment_attributes::SUN_ANGLE;
use crate::world::entity::EntityId;
use crate::world::level::LevelReader;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS};

pub struct DaylightDetectorBlock;

fn update_signal_strength(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let mut target = (level.brightness(LightLayer::Sky, pos) - level.sky_darken()).max(0);
    let mut sun_angle = level.environment_attributes().get_value_at(SUN_ANGLE, pos) * (std::f32::consts::PI / 180.0);
    if state.get(INVERTED) {
        target = 15 - target;
    } else if target > 0 {
        let offset = if sun_angle < std::f32::consts::PI { 0.0 } else { std::f32::consts::PI * 2.0 };
        sun_angle += (offset - sun_angle) * 0.2;
        target = crisp_core::mth::java_round_f32(target as f32 * crisp_core::mth::cos(sun_angle as f64));
    }
    target = target.clamp(0, 15);
    if state.get(POWER) != target {
        level.set_block(pos, state.set(POWER, target), UPDATE_ALL);
    }
}

impl BlockBehaviour for DaylightDetectorBlock {
    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if !level.may_build(player) {
            return InteractionResult::Pass;
        }
        let new_state = state.cycle(INVERTED);
        level.set_block(pos, new_state, UPDATE_CLIENTS);
        update_signal_strength(new_state, level, pos);
        InteractionResult::SUCCESS
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        state.get(POWER)
    }
}

impl ServerLevel {
    pub fn daylight_detector_tick_entity(&mut self, pos: BlockPos, state: BlockState) {
        if self.dimension_type().has_skylight && self.game_time % 20 == 0 {
            update_signal_strength(state, self, pos);
        }
    }
}
