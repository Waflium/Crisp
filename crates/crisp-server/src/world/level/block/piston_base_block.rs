use crisp_data::BlockState;
use crisp_data::properties::FACING;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct PistonBaseBlock;

impl BlockBehaviour for PistonBaseBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }
}
