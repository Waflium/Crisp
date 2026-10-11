use crisp_data::BlockState;
use crisp_data::properties::ORIENTATION;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct JigsawBlock;

impl BlockBehaviour for JigsawBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ORIENTATION, state.get(ORIENTATION).rotate(rotation.rotation()))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ORIENTATION, state.get(ORIENTATION).rotate(mirror.rotation()))
    }
}
