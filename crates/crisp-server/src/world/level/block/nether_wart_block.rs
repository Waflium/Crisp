use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AGE_3;
use crisp_data::{BlockState, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct NetherWartBlock;

impl Vegetation for NetherWartBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_NETHER_WART)
    }
}

impl BlockBehaviour for NetherWartBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let age = state.get(AGE_3);
        if age < 3 && level.random.next_int_bounded(10) == 0 {
            level.set_block(pos, state.set(AGE_3, age + 1), UPDATE_CLIENTS);
        }
    }
}
