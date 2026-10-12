use crisp_core::BlockPos;
use crisp_data::{BlockState, damage_types};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;

pub struct SlimeBlock;

impl BlockBehaviour for SlimeBlock {
    fn fall_on(&self, level: &mut ServerLevel, _state: BlockState, _pos: BlockPos, entity: EntityId, fall_distance: f64) {
        if !level.entity(entity).is_suppressing_bounce() {
            level.cause_fall_damage(entity, fall_distance, 0.0, DamageSource::new(damage_types::FALL));
        }
    }

    fn step_on(&self, level: &mut ServerLevel, _pos: BlockPos, _state: BlockState, entity: EntityId) {
        let e = level.entity_mut(entity);
        let abs_delta_y = e.delta_movement.y.abs();
        if abs_delta_y < 0.1 && !e.is_shift_key_down() {
            let scale = 0.4 + abs_delta_y * 0.2;
            e.set_delta_movement(e.delta_movement.multiply(scale, 1.0, scale));
        }
    }
}
