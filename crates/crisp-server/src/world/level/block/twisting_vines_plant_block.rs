use crisp_data::{Block, BlockState, blocks};

use super::growing_plant_body_block::BodyParts;
use super::nether_vines;

pub struct TwistingVinesPlantBlock;

impl BodyParts for TwistingVinesPlantBlock {
    fn head_block(&self) -> Block {
        blocks::TWISTING_VINES
    }

    fn head_can_grow_into(&self, state: BlockState) -> bool {
        nether_vines::is_valid_growth_state(state)
    }
}
