use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{AGE_1, IntegerProperty};
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::crop_block::{self, Crop};
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct TorchflowerCropBlock;

impl Crop for TorchflowerCropBlock {
    fn age_property(&self) -> IntegerProperty {
        AGE_1
    }

    fn max_age(&self) -> i32 {
        2
    }

    fn state_for_age(&self, block: Block, age: i32) -> BlockState {
        if age == 2 { blocks::TORCHFLOWER.default_state() } else { block.default_state().set(AGE_1, age) }
    }

    fn bonemeal_age_increase(&self, _level: &mut ServerLevel) -> i32 {
        1
    }
}

impl Vegetation for TorchflowerCropBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        crop_block::may_place_on_crop_support(state)
    }
}

impl BlockBehaviour for TorchflowerCropBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        crop_block::can_survive(self, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.random.next_int_bounded(3) != 0 {
            crop_block::random_tick(self, state, level, pos);
        }
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        !crop_block::is_max_age(self, state)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        crop_block::grow_crops(self, level, pos, state);
    }
}
