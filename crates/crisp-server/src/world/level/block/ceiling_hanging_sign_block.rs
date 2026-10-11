use crisp_data::BlockState;
use crisp_data::properties::ROTATION_16;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct CeilingHangingSignBlock;

impl BlockBehaviour for CeilingHangingSignBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ROTATION_16, rotation.rotate_index(state.get(ROTATION_16), 16))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ROTATION_16, mirror.mirror_index(state.get(ROTATION_16), 16))
    }
}
