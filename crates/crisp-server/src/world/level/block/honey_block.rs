use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::{BlockState, damage_types};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::entity::{EntityId, LevelEntity, sound_event};

pub struct HoneyBlock;

fn does_entity_do_honey_block_slide_effects(level: &ServerLevel, entity: EntityId) -> bool {
    level.is_living(entity) || matches!(level.entities.get(&entity), Some(LevelEntity::PrimedTnt(_)))
}

fn get_old_delta_y(delta_y: f64) -> f64 {
    delta_y / 0.98f32 as f64 + 0.08
}

fn get_new_delta_y(delta_y: f64) -> f64 {
    (delta_y - 0.08) * 0.98f32 as f64
}

fn is_sliding_down(level: &ServerLevel, pos: BlockPos, entity: EntityId) -> bool {
    let e = level.entity(entity);
    if e.on_ground {
        return false;
    }
    if e.y() > pos.y as f64 + 0.9375 - 1.0e-7 {
        return false;
    }
    if get_old_delta_y(e.delta_movement.y) >= -0.08 {
        return false;
    }
    let dx = (pos.x as f64 + 0.5 - e.x()).abs();
    let dz = (pos.z as f64 + 0.5 - e.z()).abs();
    let overlap_distance = 0.4375 + (e.dimensions.width / 2.0) as f64;
    dx + 1.0e-7 > overlap_distance || dz + 1.0e-7 > overlap_distance
}

fn maybe_do_slide_achievement(level: &mut ServerLevel, entity: EntityId, pos: BlockPos) {
    if level.is_player(entity) && level.game_time % 20 == 0 {
        let state = crate::world::level::BlockGetter::block_state(level, pos);
        crate::advancements::triggers::criteria_triggers::HONEY_BLOCK_SLIDE.trigger(level, entity, state);
    }
}

fn do_slide_movement(level: &mut ServerLevel, entity: EntityId) {
    let e = level.entity_mut(entity);
    let delta_movement = e.delta_movement;
    if get_old_delta_y(delta_movement.y) < -0.13 {
        let horizontal_reduction_factor = -0.05 / get_old_delta_y(delta_movement.y);
        e.set_delta_movement(Vec3::new(delta_movement.x * horizontal_reduction_factor, get_new_delta_y(-0.05), delta_movement.z * horizontal_reduction_factor));
    } else {
        e.set_delta_movement(Vec3::new(delta_movement.x, get_new_delta_y(-0.05), delta_movement.z));
    }
    level.reset_fall_distance(entity);
}

fn maybe_do_slide_effects(level: &mut ServerLevel, entity: EntityId) {
    if does_entity_do_honey_block_slide_effects(level, entity) {
        if level.random.next_int_bounded(5) == 0 {
            level.entity_play_sound(entity, sound_event("minecraft:block.honey_block.slide"), 1.0, 1.0);
        }
        if level.random.next_int_bounded(5) == 0 {
            level.broadcast_entity_event(entity, 53);
        }
    }
}

impl BlockBehaviour for HoneyBlock {
    fn fall_on(&self, level: &mut ServerLevel, state: BlockState, _pos: BlockPos, entity: EntityId, fall_distance: f64) {
        level.entity_play_sound(entity, sound_event("minecraft:block.honey_block.slide"), 1.0, 1.0);
        level.broadcast_entity_event(entity, 54);
        if level.cause_fall_damage(entity, fall_distance, 0.2, DamageSource::new(damage_types::FALL)) {
            let sound = &state.block().info().sound;
            level.entity_play_sound(entity, sound.fall_sound, sound.volume * 0.5, sound.pitch * 0.75);
        }
    }

    fn entity_inside(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if is_sliding_down(level, pos, entity) {
            maybe_do_slide_achievement(level, entity, pos);
            do_slide_movement(level, entity);
            maybe_do_slide_effects(level, entity);
        }
    }
}
