use crisp_data::BlockState;
use crisp_data::properties::ROTATION_16;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

const MAX: i32 = 15;
const ROTATIONS: i32 = MAX + 1;

pub struct SkullBlock;

impl BlockBehaviour for SkullBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ROTATION_16, rotation.rotate_index(state.get(ROTATION_16), ROTATIONS))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ROTATION_16, mirror.mirror_index(state.get(ROTATION_16), ROTATIONS))
    }
}
