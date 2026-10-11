use crisp_core::{BlockPos, Direction};
use crisp_data::properties::TIP;
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use super::vine_block::can_attach_to;
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_LIMIT};

pub struct HangingMossBlock;

fn can_stay_at_position(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    let block_state = level.block_state(pos.above());
    can_attach_to(Direction::Up, block_state) || block_state.is_block(blocks::PALE_HANGING_MOSS)
}

fn tip(level: &mut ServerLevel, pos: BlockPos) -> BlockPos {
    let mut forward_pos = pos;
    loop {
        forward_pos = forward_pos.below();
        if !level.block_state(forward_pos).is_block(blocks::PALE_HANGING_MOSS) {
            return forward_pos.above();
        }
    }
}

impl BlockBehaviour for HangingMossBlock {
    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        can_stay_at_position(level, pos)
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if !can_stay_at_position(level, pos) {
            level.schedule_tick(pos, state.block(), 1);
        }
        state.set(TIP, !level.block_state(pos.below()).is_block(state.block()))
    }

    fn tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !can_stay_at_position(level, pos) {
            level.destroy_block(pos, true, None, UPDATE_LIMIT);
        }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        let grow_pos = tip(level, pos).below();
        level.block_state(grow_pos).is_air() && !level.chunks.is_outside_build_height(grow_pos.y)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let tip_pos = tip(level, pos).below();
        if level.block_state(tip_pos).is_air() {
            level.set_block(tip_pos, state.set(TIP, true), UPDATE_ALL);
        }
    }
}
