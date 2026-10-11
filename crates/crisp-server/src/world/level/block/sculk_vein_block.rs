use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::multiface_block::MultifaceBlock;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct SculkVeinBlock;

impl BlockBehaviour for SculkVeinBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        MultifaceBlock.get_state_for_placement(block, ctx, level)
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        MultifaceBlock.can_be_replaced(state, ctx)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        MultifaceBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        MultifaceBlock.can_survive(state, level, pos)
    }
}
