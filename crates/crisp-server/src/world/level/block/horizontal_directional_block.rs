use crisp_data::BlockState;
use crisp_data::properties::HORIZONTAL_FACING;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct HorizontalDirectionalBlock;

pub fn rotate(state: BlockState, rotation: Rotation) -> BlockState {
    state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
}

pub fn mirror(state: BlockState, mirror: Mirror) -> BlockState {
    super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
}

impl BlockBehaviour for HorizontalDirectionalBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        self::mirror(state, mirror)
    }
}
