use crisp_core::random::RandomSource;
use crisp_core::{AABB, BlockPos, Vec3};
use crisp_data::{BlockState, blocks, game_rule, items};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK;
use crate::world::entity::inside_block::{InsideBlockEffectType, InsideBlockEffects};
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::phys::shapes::collision_context::CollisionContext;

const HORIZONTAL_PARTICLE_MOMENTUM_FACTOR: f32 = 0.083333336;

pub struct PowderSnowBlock;

fn random_between(level: &mut ServerLevel, min: f32, max: f32) -> f32 {
    level.random.next_float() * (max - min) + min
}

impl BlockBehaviour for PowderSnowBlock {
    fn entity_inside(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId, effects: &mut InsideBlockEffects, _is_precise: bool) {
        if !level.is_living(entity) || level.in_block_state(entity).is_block(state.block()) {
            level.make_stuck_in_block(entity, Vec3::new(0.9f32 as f64, 1.5, 0.9f32 as f64));
            let known_movement = level.known_movement(entity);
            let is_moving = known_movement.x != 0.0 || known_movement.z != 0.0;
            if is_moving && level.random.next_boolean() {
                let e = level.entity(entity);
                let (x, z) = (e.x(), e.z());
                let x_speed = random_between(level, -1.0, 1.0) * HORIZONTAL_PARTICLE_MOMENTUM_FACTOR;
                let z_speed = random_between(level, -1.0, 1.0) * HORIZONTAL_PARTICLE_MOMENTUM_FACTOR;
                level.send_particles_with_speeds(
                    "minecraft:snowflake",
                    |_| {},
                    x,
                    (pos.y + 1) as f64,
                    z,
                    1,
                    (0.0, 0.0, 0.0),
                    (x_speed as f64, 0.05f32 as f64, z_speed as f64),
                    1,
                );
            }
        }
        effects.run_before(
            InsideBlockEffectType::Extinguish,
            Box::new(move |level, e| {
                let may_interact = !level.is_player(e) || level.may_interact(e, pos);
                if level.is_on_fire(e) && (level.game_rules.get_bool(game_rule::MOB_GRIEFING) || level.is_player(e)) && may_interact {
                    level.destroy_block(pos, false, None, crate::world::level::update_flags::UPDATE_LIMIT);
                }
            }),
        );
        effects.apply(InsideBlockEffectType::Freeze);
        effects.apply(InsideBlockEffectType::Extinguish);
    }

    fn fall_on(&self, level: &mut ServerLevel, _state: BlockState, _pos: BlockPos, entity: EntityId, fall_distance: f64) {
        if !(fall_distance < 4.0) && level.is_living(entity) {
            let (small, big) = level.fall_sounds(entity);
            let fall_sound = if fall_distance < 7.0 { small } else { big };
            level.entity_play_sound(entity, sound_event(fall_sound), 1.0, 1.0);
        }
    }

    fn entity_inside_collision_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId) -> Option<Vec<AABB>> {
        let collision_shape = CollisionContext::of(level, entity).get_collision_shape(state, pos);
        if collision_shape.is_empty() { None } else { Some(collision_shape.to_aabbs()) }
    }

    fn is_bucket_pickup(&self, _state: BlockState) -> bool {
        true
    }

    fn pickup_block(&self, _player: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> Option<ItemStack> {
        level.set_block(pos, blocks::AIR.default_state(), 11);
        level.level_event(None, PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
        Some(ItemStack::new(items::POWDER_SNOW_BUCKET, 1))
    }

    fn pickup_sound(&self) -> Option<i32> {
        Some(sound_event("minecraft:item.bucket.fill_powder_snow"))
    }
}
