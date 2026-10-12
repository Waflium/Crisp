use crisp_core::{BlockPos, Vec3};
use crisp_data::{BlockState, mob_effects};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;

pub struct WebBlock;

impl BlockBehaviour for WebBlock {
    fn entity_inside(&self, _state: BlockState, level: &mut ServerLevel, _pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        let mut speed_multiplier = Vec3::new(0.25, 0.05f32 as f64, 0.25);
        if level.is_living(entity) && level.has_effect(entity, mob_effects::WEAVING) {
            speed_multiplier = Vec3::new(0.5, 0.25, 0.5);
        }
        level.make_stuck_in_block(entity, speed_multiplier);
    }
}
