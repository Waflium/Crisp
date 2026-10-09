use crisp_data::{Block, BlockState, blocks};

use super::growing_plant_body_block::BodyParts;
use super::kelp_block::{kelp_can_attach_to, kelp_can_grow_into};

pub struct KelpPlantBlock;

impl BodyParts for KelpPlantBlock {
    fn head_block(&self) -> Block {
        blocks::KELP
    }

    fn can_attach_to(&self, state: BlockState) -> bool {
        kelp_can_attach_to(state)
    }

    fn head_can_grow_into(&self, state: BlockState) -> bool {
        kelp_can_grow_into(state)
    }
}
