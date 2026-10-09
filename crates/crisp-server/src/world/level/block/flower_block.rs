use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;

pub struct FlowerBlock;

impl Vegetation for FlowerBlock {}

impl BlockBehaviour for FlowerBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }
}
