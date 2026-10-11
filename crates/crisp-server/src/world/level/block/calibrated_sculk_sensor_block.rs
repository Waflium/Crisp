use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::properties::HORIZONTAL_FACING;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::sculk_sensor_block::SculkSensorBlock;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct CalibratedSculkSensorBlock;

impl BlockBehaviour for CalibratedSculkSensorBlock {
    fn spawn_after_break(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {
        SculkSensorBlock.spawn_after_break(state, level, pos, tool, drop_experience);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
