use crisp_core::{BlockPos, Direction};
use crisp_data::properties::SNOWY;
use crisp_data::{Block, BlockState, block_tags};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};

pub fn is_snowy_setting(above_state: BlockState) -> bool {
    above_state.is(&block_tags::SNOW)
}

pub fn update_shape(state: BlockState, direction: Direction, neighbour_state: BlockState) -> BlockState {
    if direction == Direction::Up { state.set(SNOWY, is_snowy_setting(neighbour_state)) } else { state }
}

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> BlockState {
    let above_state = level.block_state(ctx.clicked_pos().above());
    block.default_state().set(SNOWY, is_snowy_setting(above_state))
}

pub struct SnowyBlock;

impl BlockBehaviour for SnowyBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(get_state_for_placement(block, ctx, level))
    }

    fn update_shape(&self, state: BlockState, _level: &mut dyn LevelAccessor, _pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        update_shape(state, direction, neighbour_state)
    }
}
