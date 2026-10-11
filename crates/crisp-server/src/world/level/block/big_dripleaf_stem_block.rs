use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{BlockState, Fluid, FluidState, block_tags, blocks};

use super::BlockBehaviour;
use super::big_dripleaf_block;
use super::vegetation_block::schedule_water_tick;
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_LIMIT};

pub struct BigDripleafStemBlock;

pub fn place(level: &mut ServerLevel, pos: BlockPos, fluid: FluidState, facing: Direction) -> bool {
    let state = blocks::BIG_DRIPLEAF_STEM.default_state().set(WATERLOGGED, fluid.is_source_of_type(Fluid::Water)).set(HORIZONTAL_FACING, facing);
    level.set_block(pos, state, UPDATE_ALL)
}

fn leaf_pos(level: &mut ServerLevel, pos: BlockPos, stem: crisp_data::Block) -> Option<BlockPos> {
    let mut forward_pos = pos;
    let forward_state = loop {
        forward_pos = forward_pos.above();
        let state = level.block_state(forward_pos);
        if !state.is_block(stem) {
            break state;
        }
    };
    forward_state.is_block(blocks::BIG_DRIPLEAF).then_some(forward_pos)
}

impl BlockBehaviour for BigDripleafStemBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> crate::world::item::ItemStack {
        crate::world::item::ItemStack::new(blocks::BIG_DRIPLEAF.item(), 1)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let below_state = level.block_state(pos.below());
        let above_state = level.block_state(pos.above());
        (below_state.is_block(state.block()) || below_state.is(&block_tags::SUPPORTS_BIG_DRIPLEAF))
            && (above_state.is_block(state.block()) || above_state.is_block(blocks::BIG_DRIPLEAF))
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if (direction == Direction::Down || direction == Direction::Up) && !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, state.block(), 1);
        }
        if state.get(WATERLOGGED) {
            schedule_water_tick(level, pos);
        }
        state
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.can_survive(state, level, pos) {
            level.destroy_block(pos, true, None, UPDATE_LIMIT);
        }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        match leaf_pos(level, pos, state.block()) {
            Some(head_pos) => big_dripleaf_block::can_place_at(level, head_pos.above()),
            None => false,
        }
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if let Some(head_pos) = leaf_pos(level, pos, state.block()) {
            let place_head_pos = head_pos.above();
            let facing = state.get(HORIZONTAL_FACING);
            let head_fluid = level.fluid_state(head_pos);
            place(level, head_pos, head_fluid, facing);
            let place_head_fluid = level.fluid_state(place_head_pos);
            big_dripleaf_block::place(level, place_head_pos, place_head_fluid, facing);
        }
    }
}
