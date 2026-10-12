use crisp_core::BlockPos;
use crisp_data::{Block, BlockState, damage_types};

use super::BlockBehaviour;
use super::rotated_pillar_block::RotatedPillarBlock;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;

pub struct HayBlock;

impl BlockBehaviour for HayBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        RotatedPillarBlock.get_state_for_placement(block, ctx, level)
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        RotatedPillarBlock.rotate(state, rotation)
    }

    fn fall_on(&self, level: &mut ServerLevel, _state: BlockState, _pos: BlockPos, entity: EntityId, fall_distance: f64) {
        level.cause_fall_damage(entity, fall_distance, 0.2, DamageSource::new(damage_types::FALL));
    }
}
