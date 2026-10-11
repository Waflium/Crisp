use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState, Fluid, SupportType, blocks};

use super::BlockBehaviour;
use super::vegetation_block::schedule_water_tick;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};

pub struct HangingRootsBlock;

impl BlockBehaviour for HangingRootsBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let fluid = level.fluid_state(ctx.clicked_pos());
        Some(block.default_state().set(WATERLOGGED, fluid.fluid == Fluid::Water))
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.above()).is_face_sturdy(Direction::Down, SupportType::Full)
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Up && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        if state.get(WATERLOGGED) {
            schedule_water_tick(level, pos);
        }
        state
    }
}
