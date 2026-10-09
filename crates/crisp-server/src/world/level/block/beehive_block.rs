use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::properties::LEVEL_HONEY;

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::values::block_state_properties_nbt;
use crate::world::item::component::{self as dc};

pub struct BeehiveBlock;

impl BlockBehaviour for BeehiveBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        let mut stack = ItemStack::new(state.block().item(), 1);
        if include_data {
            stack.set(dc::BLOCK_STATE, Some(block_state_properties_nbt(&[("honey_level", &state.get(LEVEL_HONEY).to_string())])));
        }
        stack
    }
}
