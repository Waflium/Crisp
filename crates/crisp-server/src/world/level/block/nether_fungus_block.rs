use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation, fields, is_in, tag_field};
use crate::server::level::ServerLevel;
use crate::world::level::LevelReader;

pub struct NetherFungusBlock {
    support_blocks: &'static [i32],
}

impl NetherFungusBlock {
    pub fn new(block: Block) -> NetherFungusBlock {
        NetherFungusBlock { support_blocks: tag_field(&fields(block), "supportBlocks") }
    }
}

impl Vegetation for NetherFungusBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        is_in(state, self.support_blocks)
    }
}

impl BlockBehaviour for NetherFungusBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }
}
