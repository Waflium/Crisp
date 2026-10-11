use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::properties::{HORIZONTAL_FACING, LEVEL_HONEY};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::values::block_state_properties_nbt;
use crate::world::item::component::{self as dc};

pub struct BeehiveBlock;

impl BlockBehaviour for BeehiveBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        let mut stack = ItemStack::new(state.block().item(), 1);
        if include_data {
            stack.set(dc::BLOCK_STATE, Some(block_state_properties_nbt(&[("honey_level", &state.get(LEVEL_HONEY).to_string())])));
        }
        stack
    }
}
