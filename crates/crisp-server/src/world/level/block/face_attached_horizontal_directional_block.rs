use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{ATTACH_FACE, AttachFace, HORIZONTAL_FACING};
use crisp_data::{Block, BlockState, blocks};

use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub fn get_connected_direction(state: BlockState) -> Direction {
    match state.get(ATTACH_FACE) {
        AttachFace::Ceiling => Direction::Down,
        AttachFace::Floor => Direction::Up,
        AttachFace::Wall => state.get(HORIZONTAL_FACING),
    }
}

pub fn can_attach(level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> bool {
    let relative = pos.relative(direction);
    level.block_state(relative).is_face_sturdy_full(direction.opposite())
}

pub fn can_survive(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
    can_attach(level, pos, get_connected_direction(state).opposite())
}

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
    for direction in ctx.nearest_looking_directions() {
        let state = if direction.axis() == Axis::Y {
            let face = if direction == Direction::Up { AttachFace::Ceiling } else { AttachFace::Floor };
            block.default_state().set(ATTACH_FACE, face).set(HORIZONTAL_FACING, ctx.horizontal_direction())
        } else {
            block.default_state().set(ATTACH_FACE, AttachFace::Wall).set(HORIZONTAL_FACING, direction.opposite())
        };
        if super::of(state).can_survive(state, level, ctx.clicked_pos()) {
            return Some(state);
        }
    }
    None
}

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> BlockState {
    if get_connected_direction(state).opposite() == direction && !super::of(state).can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
}

pub struct FaceAttachedHorizontalDirectionalBlock;

impl super::BlockBehaviour for FaceAttachedHorizontalDirectionalBlock {
    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        can_survive(state, level, pos)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        get_state_for_placement(block, ctx, level)
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
