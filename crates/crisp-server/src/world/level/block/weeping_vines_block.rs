use crisp_data::{Block, BlockState, blocks};

use super::growing_plant_head_block::HeadParts;
use super::nether_vines;
use crate::server::level::ServerLevel;

pub struct WeepingVinesBlock;

impl HeadParts for WeepingVinesBlock {
    fn body_block(&self) -> Block {
        blocks::WEEPING_VINES_PLANT
    }

    fn can_grow_into(&self, state: BlockState) -> bool {
        nether_vines::is_valid_growth_state(state)
    }

    fn blocks_to_grow_when_bonemealed(&self, level: &mut ServerLevel) -> i32 {
        nether_vines::blocks_to_grow_when_bonemealed(level)
    }
}
