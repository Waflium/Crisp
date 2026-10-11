use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation, at_most_matched};
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct MushroomBlock;

fn random_offset(level: &mut ServerLevel, pos: BlockPos) -> BlockPos {
    let dx = level.random.next_int_bounded(3) - 1;
    let dy = level.random.next_int_bounded(2) - level.random.next_int_bounded(2);
    let dz = level.random.next_int_bounded(3) - 1;
    pos.offset(dx, dy, dz)
}

impl Vegetation for MushroomBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        state.is_solid_render()
    }
}

impl BlockBehaviour for MushroomBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let below_pos = pos.below();
        let below = level.block_state(below_pos);
        if below.is(&block_tags::OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT) {
            return true;
        }
        level.raw_brightness(pos, 0) < 13 && self.may_place_on(below, level, below_pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, mut pos: BlockPos) {
        if level.random.next_int_bounded(25) != 0 {
            return;
        }
        if !at_most_matched(level, pos.offset(-4, -1, -4), pos.offset(4, 1, 4), state.block(), 4) {
            return;
        }
        let mut offset = random_offset(level, pos);
        for _ in 0..4 {
            if level.block_state(offset).is_air() && self.can_survive(state, level, offset) {
                pos = offset;
            }
            offset = random_offset(level, pos);
        }
        if level.block_state(offset).is_air() && self.can_survive(state, level, offset) {
            level.set_block(offset, state, UPDATE_CLIENTS);
        }
    }
}
