use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::vegetation_block::{block_field, fields};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_ALL;

pub struct PathBlock {
    base_block: Block,
}

impl PathBlock {
    pub fn new(block: Block) -> PathBlock {
        PathBlock { base_block: block_field(&fields(block), "baseBlock") }
    }

    pub fn turn_to_base_block(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let new_state = super::push_entities_up(state, self.base_block.default_state(), level, pos);
        level.set_block(pos, new_state, UPDATE_ALL);
    }
}

impl BlockBehaviour for PathBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let default = block.default_state();
        let pos = ctx.clicked_pos();
        if !self.can_survive(default, level, pos) {
            Some(super::push_entities_up(default, crisp_data::blocks::DIRT.default_state(), level, pos))
        } else {
            Some(default)
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Up && !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, state.block(), 1);
        }
        state
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        self.turn_to_base_block(state, level, pos);
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let above_state = level.block_state(pos.above());
        !above_state.is_solid() || above_state.block().is_instance_of("FenceGateBlock")
    }
}
