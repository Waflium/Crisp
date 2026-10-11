use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;
use crisp_data::properties::STAGE;

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::LevelReader;
use crate::world::level::update_flags::UPDATE_NONE;

pub fn advance_tree(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    if state.get(STAGE) == 0 {
        level.set_block(pos, state.cycle(STAGE), UPDATE_NONE);
    }
}

pub struct SaplingBlock;

impl Vegetation for SaplingBlock {}

impl BlockBehaviour for SaplingBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.max_local_raw_brightness(pos.above(), level.sky_darken()) >= 9 && level.random.next_int_bounded(7) == 0 {
            advance_tree(level, pos, state);
        }
    }
}
