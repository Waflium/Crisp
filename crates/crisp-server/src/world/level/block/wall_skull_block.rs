use crisp_core::BlockPos;
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::abstract_skull_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub struct WallSkullBlock;

impl BlockBehaviour for WallSkullBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = abstract_skull_block::get_state_for_placement(block, ctx, level);
        let pos = ctx.clicked_pos();
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() {
                state = state.set(HORIZONTAL_FACING, direction.opposite());
                let neighbour = level.block_state(pos.relative(direction));
                if !super::of(neighbour).can_be_replaced(neighbour, ctx) {
                    return Some(state);
                }
            }
        }
        None
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        abstract_skull_block::neighbor_changed(state, level, pos);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
