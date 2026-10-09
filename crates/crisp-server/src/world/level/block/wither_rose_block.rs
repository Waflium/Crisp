use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;

pub struct WitherRoseBlock;

impl Vegetation for WitherRoseBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_WITHER_ROSE)
    }
}

impl BlockBehaviour for WitherRoseBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }
}
