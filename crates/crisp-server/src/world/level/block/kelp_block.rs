use crisp_data::{Block, BlockState, block_tags, blocks, fluid_tags};

use super::growing_plant_head_block::HeadParts;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub struct KelpBlock;

pub fn kelp_can_attach_to(state: BlockState) -> bool {
    !state.is(&block_tags::CANNOT_SUPPORT_KELP)
}

pub fn kelp_can_grow_into(state: BlockState) -> bool {
    state.is_block(blocks::WATER)
}

impl HeadParts for KelpBlock {
    fn body_block(&self) -> Block {
        blocks::KELP_PLANT
    }

    fn can_attach_to(&self, state: BlockState) -> bool {
        kelp_can_attach_to(state)
    }

    fn can_grow_into(&self, state: BlockState) -> bool {
        kelp_can_grow_into(state)
    }

    fn blocks_to_grow_when_bonemealed(&self, _level: &mut ServerLevel) -> i32 {
        1
    }

    fn may_place(&self, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> bool {
        let fluid = level.fluid_state(ctx.clicked_pos());
        fluid.is(&fluid_tags::WATER) && fluid.amount == 8
    }
}
