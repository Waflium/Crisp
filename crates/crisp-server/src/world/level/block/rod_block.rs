use crisp_data::BlockState;
use crisp_data::properties::FACING;

use super::mirror::Mirror;
use super::rotation::Rotation;

pub fn rotate(state: BlockState, rotation: Rotation) -> BlockState {
    state.set(FACING, rotation.rotate(state.get(FACING)))
}

pub fn mirror(state: BlockState, mirror: Mirror) -> BlockState {
    state.set(FACING, mirror.mirror(state.get(FACING)))
}
