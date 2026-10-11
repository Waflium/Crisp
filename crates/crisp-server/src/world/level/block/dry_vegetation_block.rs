use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::world::level::{LevelAccessor, LevelReader};

pub struct DryVegetationBlock;

impl Vegetation for DryVegetationBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_DRY_VEGETATION)
    }
}

impl BlockBehaviour for DryVegetationBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }
}
