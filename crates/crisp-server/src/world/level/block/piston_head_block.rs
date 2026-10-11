use crisp_core::BlockPos;
use crisp_data::properties::{FACING, PISTON_TYPE, PistonType};
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct PistonHeadBlock;

impl BlockBehaviour for PistonHeadBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        let base = if state.get(PISTON_TYPE) == PistonType::Sticky { blocks::STICKY_PISTON } else { blocks::PISTON };
        ItemStack::new(base.item(), 1)
    }
}
