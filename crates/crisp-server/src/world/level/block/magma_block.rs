use crisp_core::BlockPos;
use crisp_data::{BlockState, damage_types};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;

pub struct MagmaBlock;

impl BlockBehaviour for MagmaBlock {
    fn step_on(&self, level: &mut ServerLevel, _pos: BlockPos, _state: BlockState, entity: EntityId) {
        if !level.entity(entity).is_shift_key_down() && level.is_living(entity) {
            level.hurt(entity, DamageSource::new(damage_types::HOT_FLOOR), 1.0);
        }
    }
}
