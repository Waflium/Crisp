use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub struct WallTorchBlock;

pub fn can_survive_facing(level: &mut ServerLevel, pos: BlockPos, facing: Direction) -> bool {
    let relative_pos = pos.relative(facing.opposite());
    level.block_state(relative_pos).is_face_sturdy_full(facing)
}

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> BlockState {
    if direction.opposite() == state.get(HORIZONTAL_FACING) && !super::of(state).can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
}

impl BlockBehaviour for WallTorchBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = block.default_state();
        let pos = ctx.clicked_pos();
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() {
                state = state.set(HORIZONTAL_FACING, direction.opposite());
                if super::of(state).can_survive(state, level, pos) {
                    return Some(state);
                }
            }
        }
        None
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        can_survive_facing(level, pos, state.get(HORIZONTAL_FACING))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        update_shape(state, level, pos, direction)
    }
}
