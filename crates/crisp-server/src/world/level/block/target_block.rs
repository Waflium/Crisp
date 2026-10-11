use crisp_core::BlockPos;
use crisp_data::properties::POWER;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::level::LevelReader;
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_KNOWN_SHAPE};

pub struct TargetBlock {
    pub block: Block,
}

impl BlockBehaviour for TargetBlock {
    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(POWER) != 0 {
            level.set_block_simple(pos, state.set(POWER, 0));
        }
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        state.get(POWER)
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !state.is_block(old_state.block()) && state.get(POWER) > 0 && !level.has_scheduled_tick(pos, self.block) {
            level.set_block(pos, state.set(POWER, 0), UPDATE_CLIENTS | UPDATE_KNOWN_SHAPE);
        }
    }
}
