use crisp_data::BlockState;

use super::BlockBehaviour;
use super::rotated_pillar_block;
use super::rotation::Rotation;

pub struct InfestedRotatedPillarBlock;

impl BlockBehaviour for InfestedRotatedPillarBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        rotated_pillar_block::rotate_pillar(state, rotation)
    }
}
