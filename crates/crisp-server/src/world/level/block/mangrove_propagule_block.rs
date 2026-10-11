use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{AGE_4, HANGING, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, block_tags, blocks};

use super::BlockBehaviour;
use super::sapling_block::advance_tree;
use super::vegetation_block::{self, Vegetation, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub fn new_hanging_propagule(age: i32) -> BlockState {
    blocks::MANGROVE_PROPAGULE.default_state().set(HANGING, true).set(AGE_4, age)
}

fn is_hanging(state: BlockState) -> bool {
    state.get(HANGING)
}

fn is_fully_grown(state: BlockState) -> bool {
    state.get(AGE_4) == 4
}

pub struct MangrovePropaguleBlock;

impl Vegetation for MangrovePropaguleBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_MANGROVE_PROPAGULE)
    }
}

impl BlockBehaviour for MangrovePropaguleBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let replaced_fluid = level.fluid_state(ctx.clicked_pos());
        Some(block.default_state().set(WATERLOGGED, replaced_fluid.fluid == Fluid::Water).set(AGE_4, 4))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if state.get(WATERLOGGED) {
            schedule_water_tick(level, pos);
        }
        if direction == Direction::Up && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        if is_hanging(state) {
            level.block_state(pos.above()).is(&block_tags::SUPPORTS_HANGING_MANGROVE_PROPAGULE)
        } else {
            vegetation_block::can_survive(self, level, pos)
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !is_hanging(state) {
            if level.random.next_int_bounded(7) == 0 {
                advance_tree(level, pos, state);
            }
        } else if !is_fully_grown(state) {
            level.set_block(pos, state.cycle(AGE_4), UPDATE_CLIENTS);
        }
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        is_hanging(state) && !is_fully_grown(state)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        is_hanging(state) && !is_fully_grown(state)
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if is_hanging(state) && !is_fully_grown(state) {
            level.set_block(pos, state.cycle(AGE_4), UPDATE_CLIENTS);
        }
    }
}
