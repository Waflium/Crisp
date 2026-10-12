use crisp_data::BlockState;
use crisp_data::properties::HORIZONTAL_FACING;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct EndPortalFrameBlock;

impl BlockBehaviour for EndPortalFrameBlock {
    fn get_analog_output_signal(
        &self,
        state: BlockState,
        _level: &mut crate::server::level::ServerLevel,
        _pos: crisp_core::BlockPos,
        _direction: crisp_core::Direction,
    ) -> i32 {
        if state.get(crisp_data::properties::EYE) { 15 } else { 0 }
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
