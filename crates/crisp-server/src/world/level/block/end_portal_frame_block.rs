use std::sync::LazyLock;

use crisp_core::Direction;
use crisp_data::properties::{EYE, HORIZONTAL_FACING};
use crisp_data::{BlockState, blocks};

use super::state::pattern::block_pattern::{BlockInWorldPredicate, BlockPattern, BlockPatternBuilder, has_state};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct EndPortalFrameBlock;

fn frame_with_eye(facing: Direction) -> BlockInWorldPredicate {
    has_state(move |state| state.is_block(blocks::END_PORTAL_FRAME) && state.get(EYE) && state.get(HORIZONTAL_FACING) == facing)
}

static PORTAL_SHAPE: LazyLock<BlockPattern> = LazyLock::new(|| {
    BlockPatternBuilder::start()
        .aisle(&["?vvv?", ">???<", ">???<", ">???<", "?^^^?"])
        .where_('?', Box::new(|_| true))
        .where_('^', frame_with_eye(Direction::South))
        .where_('>', frame_with_eye(Direction::West))
        .where_('v', frame_with_eye(Direction::North))
        .where_('<', frame_with_eye(Direction::East))
        .build()
});

pub fn get_or_create_portal_shape() -> &'static BlockPattern {
    &PORTAL_SHAPE
}

impl BlockBehaviour for EndPortalFrameBlock {
    fn get_analog_output_signal(
        &self,
        state: BlockState,
        _level: &mut crate::server::level::ServerLevel,
        _pos: crisp_core::BlockPos,
        _direction: crisp_core::Direction,
    ) -> i32 {
        if state.get(EYE) { 15 } else { 0 }
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
