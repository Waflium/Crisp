use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::{AGE_7, HORIZONTAL_FACING};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::crop_block;
use super::vegetation_block::{self, Vegetation, block_field, fields, is_in, tag_field};
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS};

pub struct StemBlock {
    fruit: Block,
    attached_stem: Block,
    stem_support_blocks: &'static [i32],
    fruit_support_blocks: &'static [i32],
}

impl StemBlock {
    pub fn new(block: Block) -> StemBlock {
        let fields = fields(block);
        StemBlock {
            fruit: block_field(&fields, "fruit"),
            attached_stem: block_field(&fields, "attachedStem"),
            stem_support_blocks: tag_field(&fields, "stemSupportBlocks"),
            fruit_support_blocks: tag_field(&fields, "fruitSupportBlocks"),
        }
    }
}

impl Vegetation for StemBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        is_in(state, self.stem_support_blocks)
    }
}

impl BlockBehaviour for StemBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.raw_brightness(pos, 0) < 9 {
            return;
        }
        let growth_speed = crop_block::growth_speed(state.block(), level, pos);
        if level.random.next_int_bounded((25.0f32 / growth_speed) as i32 + 1) != 0 {
            return;
        }
        let age = state.get(AGE_7);
        if age < 7 {
            level.set_block(pos, state.set(AGE_7, age + 1), UPDATE_CLIENTS);
        } else {
            let direction = Direction::PLANE_HORIZONTAL[level.random.next_int_bounded(4) as usize];
            let relative = pos.relative(direction);
            let state_below = level.block_state(relative.below());
            if level.block_state(relative).is_air() && is_in(state_below, self.fruit_support_blocks) {
                level.set_block(relative, self.fruit.default_state(), UPDATE_ALL);
                level.set_block(pos, self.attached_stem.default_state().set(HORIZONTAL_FACING, direction), UPDATE_ALL);
            }
        }
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        state.get(AGE_7) != 7
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let age = 7.min(state.get(AGE_7) + mth::next_int(&mut level.random, 2, 5));
        let new_state = state.set(AGE_7, age);
        level.set_block(pos, new_state, UPDATE_CLIENTS);
        if age == 7 {
            self.random_tick(new_state, level, pos);
        }
    }
}
