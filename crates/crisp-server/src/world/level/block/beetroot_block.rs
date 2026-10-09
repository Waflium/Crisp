use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, mth};
use crisp_data::BlockState;
use crisp_data::properties::{AGE_3, IntegerProperty};

use super::BlockBehaviour;
use super::crop_block::{self, Crop};
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;

pub struct BeetrootBlock;

impl Crop for BeetrootBlock {
    fn age_property(&self) -> IntegerProperty {
        AGE_3
    }

    fn max_age(&self) -> i32 {
        3
    }

    fn bonemeal_age_increase(&self, level: &mut ServerLevel) -> i32 {
        mth::next_int(&mut level.random, 2, 5) / 3
    }
}

impl Vegetation for BeetrootBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> bool {
        crop_block::may_place_on_crop_support(state)
    }
}

impl BlockBehaviour for BeetrootBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
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
