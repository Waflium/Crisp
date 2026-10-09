use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{AGE_7, HORIZONTAL_FACING};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation, block_field, fields, is_in, tag_field};
use crate::server::level::ServerLevel;

pub struct AttachedStemBlock {
    fruit: Block,
    stem: Block,
    support_blocks: &'static [i32],
}

impl AttachedStemBlock {
    pub fn new(block: Block) -> AttachedStemBlock {
        let fields = fields(block);
        AttachedStemBlock { fruit: block_field(&fields, "fruit"), stem: block_field(&fields, "stem"), support_blocks: tag_field(&fields, "supportBlocks") }
    }
}

impl Vegetation for AttachedStemBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> bool {
        is_in(state, self.support_blocks)
    }
}

impl BlockBehaviour for AttachedStemBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> crate::world::item::ItemStack {
        crate::world::item::ItemStack::new(self.stem.item(), 1)
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        if !neighbour_state.is_block(self.fruit) && direction == state.get(HORIZONTAL_FACING) {
            return self.stem.default_state().try_set(AGE_7, 7);
        }
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }
}
