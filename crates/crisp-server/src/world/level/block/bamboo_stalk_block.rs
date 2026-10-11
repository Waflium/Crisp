use crisp_core::BlockPos;
use crisp_data::{BlockState, block_tags};

use super::BlockBehaviour;
use crate::world::level::LevelReader;

pub struct BambooStalkBlock;

impl BlockBehaviour for BambooStalkBlock {
    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.below()).is(&block_tags::SUPPORTS_BAMBOO)
    }
}
