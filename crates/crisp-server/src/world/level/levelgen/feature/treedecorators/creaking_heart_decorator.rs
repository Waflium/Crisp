use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::Direction;
use crisp_core::random::RandomSource;
use crisp_data::properties::{CREAKING_HEART_STATE, CreakingHeartState, NATURAL};
use crisp_data::{block_tags, blocks};

use super::tree_decorator::Context;
use crate::util;

pub fn place<R: RandomSource>(probability: f32, context: &mut Context<impl WorldGenLevel, R>) {
    if context.logs.is_empty() {
        return;
    }
    if context.random.next_float() >= probability {
        return;
    }
    let mut heart_placements = context.logs.clone();
    util::shuffle(&mut heart_placements, context.random);
    let target_pos = heart_placements.into_iter().find(|&pos| Direction::VALUES.into_iter().all(|dir| context.check_block(pos.relative(dir), |state| state.is(&block_tags::LOGS))));
    if let Some(target_pos) = target_pos {
        context.set_block(target_pos, blocks::CREAKING_HEART.default_state().set(CREAKING_HEART_STATE, CreakingHeartState::Dormant).set(NATURAL, true));
    }
}
