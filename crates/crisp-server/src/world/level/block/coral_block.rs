use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, fluid_tags};

use super::BlockBehaviour;
use super::vegetation_block::{block_field, fields};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct CoralBlock {
    block: Block,
    dead_block: Block,
}

impl CoralBlock {
    pub fn new(block: Block) -> CoralBlock {
        CoralBlock { block, dead_block: block_field(&fields(block), "deadBlock") }
    }

    fn scan_for_water(&self, level: &mut dyn LevelReader, block_pos: BlockPos) -> bool {
        Direction::VALUES.iter().any(|&direction| level.fluid_state(block_pos.relative(direction)).is(&fluid_tags::WATER))
    }
}

impl BlockBehaviour for CoralBlock {
    fn tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.scan_for_water(level, pos) {
            level.set_block(pos, self.dead_block.default_state(), UPDATE_CLIENTS);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if !self.scan_for_water(level, pos) {
            let delay = 60 + level.get_random().next_int_bounded(40);
            level.schedule_tick(pos, self.block, delay);
        }
        state
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        if !self.scan_for_water(level, pos) {
            let delay = 60 + level.random.next_int_bounded(40);
            level.schedule_tick(pos, self.block, delay);
        }
        Some(block.default_state())
    }
}
