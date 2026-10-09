use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::properties::LEVEL;

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::values::block_state_properties_nbt;
use crate::world::item::component::{self as dc};

pub struct LightBlock;

pub fn set_light_on_stack(mut stack: ItemStack, light_level: i32) -> ItemStack {
    stack.set(dc::BLOCK_STATE, Some(block_state_properties_nbt(&[("level", &light_level.to_string())])));
    stack
}

impl BlockBehaviour for LightBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        set_light_on_stack(ItemStack::new(state.block().item(), 1), state.get(LEVEL))
    }
}
