use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::BlockBehaviour;
use super::infested_block;
use super::rotated_pillar_block;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct InfestedRotatedPillarBlock;

impl BlockBehaviour for InfestedRotatedPillarBlock {
    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, _drop_experience: bool) {
        infested_block::spawn_after_break(level, pos, tool);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        rotated_pillar_block::rotate_pillar(state, rotation)
    }
}
