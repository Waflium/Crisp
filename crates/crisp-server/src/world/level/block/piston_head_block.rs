use crisp_core::BlockPos;
use crisp_data::properties::{PISTON_TYPE, PistonType};
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct PistonHeadBlock;

impl BlockBehaviour for PistonHeadBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        let base = if state.get(PISTON_TYPE) == PistonType::Sticky { blocks::STICKY_PISTON } else { blocks::PISTON };
        ItemStack::new(base.item(), 1)
    }
}
