use crisp_core::BlockPos;
use crisp_data::{BlockState, items};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct BambooSaplingBlock;

impl BlockBehaviour for BambooSaplingBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> ItemStack {
        ItemStack::new(items::BAMBOO, 1)
    }
}
