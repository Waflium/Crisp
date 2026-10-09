use crisp_core::BlockPos;
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct CandleCakeBlock;

impl BlockBehaviour for CandleCakeBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> ItemStack {
        ItemStack::new(blocks::CAKE.item(), 1)
    }
}
