use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::properties::{HORIZONTAL_FACING, OPEN};
use crisp_data::packets::play::clientbound;
use crisp_data::{block_tags, blocks, fluid_tags, items, mob_effects};

use super::attributes::{Attribute, AttributeMap, AttributeModifier, PLAYER_ATTRIBUTES, SPEED_MODIFIER_SPRINTING};
use super::data::Pose;
use super::movement::compute_modified_friction;
use super::{EntityDimensions, EntityId, LevelEntity, MoverType, flags};
use crate::server::level::ServerLevel;
use crate::network::connection::encode;
use crate::util::Prediction;
use crate::world::InteractionHand;
use crate::world::item::ItemStack;
use crate::world::item::component::swing_animation::SwingAnimation;
use crate::world::level::BlockGetter;

const SPEED_MODIFIER_POWDER_SNOW_ID: &str = "minecraft:powder_snow";
const TICKS_REQUIRED_TO_FREEZE: i32 = 140;

pub struct LivingEntity {
    pub attributes: AttributeMap,
    pub jumping: bool,
    pub xxa: f32,
    pub yya: f32,
    pub zza: f32,
    pub speed: f32,
    pub no_jump_delay: i32,
    pub last_climbable_pos: Option<BlockPos>,
    pub current_impulse_impact_pos: Option<Vec3>,
    pub current_impulse_context_reset_grace_time: i32,
    pub hurt_time: i32,
    pub hurt_duration: i32,
    pub death_time: i32,
    pub damage_cooldown_time: i32,
    pub last_hurt: f32,
    pub dead: bool,
    pub last_damage_source: Option<crate::world::damagesource::DamageSource>,
    pub last_damage_stamp: i64,
    pub combat_tracker: crate::world::damagesource::combat_tracker::CombatTracker,
    pub hurt_dir: f32,
    pub use_item: Option<crate::world::item::ItemStack>,
    pub use_item_remaining: i32,
    pub last_equipment_items: [Option<crate::world::item::ItemStack>; 8],
    pub swing_state: SwingState,
    pub active_effects: Vec<crate::world::effect::MobEffectInstance>,
    pub effects_dirty: bool,
    pub effects_mod_count: u32,
}

impl LivingEntity {
    pub fn player() -> LivingEntity {
        LivingEntity {
            attributes: AttributeMap::new(&PLAYER_ATTRIBUTES),
            jumping: false,
            xxa: 0.0,
            yya: 0.0,
            zza: 0.0,
            speed: 0.0,
            no_jump_delay: 0,
            last_climbable_pos: None,
            current_impulse_impact_pos: None,
            current_impulse_context_reset_grace_time: 0,
            hurt_time: 0,
            hurt_duration: 0,
            death_time: 0,
            damage_cooldown_time: 0,
            last_hurt: 0.0,
            dead: false,
            last_damage_source: None,
            last_damage_stamp: 0,
            combat_tracker: Default::default(),
            hurt_dir: 0.0,
            use_item: None,
            use_item_remaining: 0,
            last_equipment_items: Default::default(),
            swing_state: SwingState::default(),
            active_effects: Vec::new(),
            effects_dirty: true,
            effects_mod_count: 0,
        }
    }

    pub fn reset_current_impulse_context(&mut self) {
        self.current_impulse_context_reset_grace_time = 0;
        self.current_impulse_impact_pos = None;
    }

    pub fn try_reset_current_impulse_context(&mut self) {
        if self.current_impulse_context_reset_grace_time == 0 {
            self.reset_current_impulse_context();
        }
    }

    pub fn is_in_post_impulse_grace_time(&self) -> bool {
        self.current_impulse_context_reset_grace_time > 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwingDescription {
    pub hand: InteractionHand,
    pub animation: SwingAnimation,
    pub duration_ticks: i32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct SwingState {
    pub current_swing: Option<SwingDescription>,
    ticks: i32,
    old_animation: f32,
    pub animation: f32,
}

impl SwingState {
    pub fn start_if_able(&mut self, hand: InteractionHand, animation: SwingAnimation, duration_ticks: i32) -> bool {
        if self.current_swing.is_some_and(|s| self.ticks <= s.duration_ticks / 2) && self.ticks > 0 {
            return false;
        }
        let old_swing = self.current_swing;
        let swing = SwingDescription { hand, animation, duration_ticks };
        self.current_swing = Some(swing);
        if old_swing != Some(swing) {
            self.old_animation = 0.0;
            self.animation = 0.0;
        }
        self.ticks = 0;
        true
    }

    pub fn tick(&mut self) {
        self.old_animation = self.animation;
        match self.current_swing {
            Some(swing) => {
                if swing.duration_ticks > 0 {
                    self.animation = (self.ticks as f32 / swing.duration_ticks as f32).min(1.0);
                }
                let ticks = self.ticks;
                self.ticks += 1;
                if ticks > swing.duration_ticks {
                    self.current_swing = None;
                    self.animation = 0.0;
                }
            }
            None => self.animation = 0.0,
        }
    }
}

pub fn player_dimensions(pose: Pose) -> EntityDimensions {
    match pose {
        Pose::Sleeping => EntityDimensions { width: 0.2, height: 0.2, eye_height: 0.2 },
        Pose::FallFlying | Pose::Swimming | Pose::SpinAttack => EntityDimensions { width: 0.6, height: 0.6, eye_height: 0.4 },
        Pose::Crouching => EntityDimensions { width: 0.6, height: 1.5, eye_height: 1.27 },
        Pose::Dying => EntityDimensions { width: 0.2, height: 0.2, eye_height: 1.62 },
        _ => EntityDimensions { width: 0.6, height: 1.8, eye_height: 1.62 },
    }
}

impl ServerLevel {
    pub fn swing(&mut self, id: EntityId, hand: InteractionHand, animation: SwingAnimation, send_to_swinging_entity: bool) -> bool {
        let duration = animation.duration;
        if !self.player_mut(id).living.swing_state.start_if_able(hand, animation, duration) {
            return false;
        }
        let packet = encode(clientbound::SWING_ANIMATION, |w| {
            w.varint(id);
            w.varint(hand as i32);
            animation.write(w);
        });
        if send_to_swinging_entity {
            self.send_to_tracking_players_and_self(id, &packet);
        } else {
            self.send_to_tracking_players(id, &packet);
        }
        true
    }

    pub fn player_drop(&mut self, player: EntityId, stack: ItemStack, thrown_from_hand: bool, prediction: Prediction) {
        if stack.is_empty() {
            return;
        }
        self.swing(player, InteractionHand::MainHand, SwingAnimation::DEFAULT, prediction != Prediction::Predicted);
        let item = self.create_item_stack_to_drop(player, stack, false, thrown_from_hand);
        self.add_fresh_entity(LevelEntity::Item(item));
    }

    pub fn living_speed(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MovementSpeed) as f32
    }

    pub fn set_sprinting(&mut self, id: EntityId, sprinting: bool) {
        let p = self.player_mut(id);
        p.entity.set_shared_flag(flags::SPRINTING, sprinting);
        let speed = p.living.attributes.instance(Attribute::MovementSpeed).expect("players have movement speed");
        speed.remove_modifier(&SPEED_MODIFIER_SPRINTING.id);
        if sprinting {
            speed.add_transient_modifier(SPEED_MODIFIER_SPRINTING);
        }
    }

    pub fn player_do_tick(&mut self, id: EntityId) {
        if !self.is_spectator(id) || !self.touching_unloaded_chunk(id) {
            self.player_tick(id);
            self.tick_food(id);
        }
        self.send_health_if_changed(id);
    }

    fn player_tick(&mut self, id: EntityId) {
        let spectator = self.is_spectator(id);
        self.entity_mut(id).no_physics = spectator;
        if spectator {
            self.set_on_ground(id, false);
        }
        self.player_mut(id).living.swing_state.tick();
        self.entity_base_tick(id);
        self.living_base_tick(id);
        self.updating_using_item(id);
        self.detect_equipment_updates(id);
        if self.entity(id).tick_count % 20 == 0 {
            self.recheck_combat_status(id);
        }
        if !self.entity(id).is_removed() {
            self.player_ai_step(id);
        }
        self.refresh_dirty_attributes(id);
        let living = &mut self.player_mut(id).living;
        if living.current_impulse_context_reset_grace_time > 0 {
            living.current_impulse_context_reset_grace_time -= 1;
        }
        let e = self.entity_mut(id);
        let nx = mth::clamp(e.x(), -2.9999999e7, 2.9999999e7);
        let nz = mth::clamp(e.z(), -2.9999999e7, 2.9999999e7);
        if nx != e.x() || nz != e.z() {
            let y = e.y();
            e.set_pos(Vec3::new(nx, y, nz));
        }
        self.update_player_pose(id);
    }

    fn player_ai_step(&mut self, id: EntityId) {
        self.tick_regeneration(id);
        if self.player(id).abilities_flying() {
            self.reset_fall_distance(id);
        }
        self.living_ai_step(id);
        let speed = self.living_speed(id);
        let p = self.player_mut(id);
        p.entity.y_head_rot = p.entity.y_rot;
        p.living.speed = speed;
        if p.health > 0.0 && !self.is_spectator(id) {
            self.pick_up_items(id);
        }
    }

    fn living_ai_step(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        if p.living.no_jump_delay > 0 {
            p.living.no_jump_delay -= 1;
        }
        let movement = p.entity.delta_movement;
        let (mut dx, mut dy, mut dz) = (movement.x, movement.y, movement.z);
        if movement.horizontal_distance_sqr() < 9.0e-6 {
            dx = 0.0;
            dz = 0.0;
        }
        if movement.y.abs() < 0.003 {
            dy = 0.0;
        }
        p.entity.set_delta_movement(Vec3::new(dx, dy, dz));
        p.living.xxa *= 0.98;
        p.living.zza *= 0.98;
        if p.health <= 0.0 {
            p.living.jumping = false;
            p.living.xxa = 0.0;
            p.living.zza = 0.0;
        }
        let affected_by_fluids = !p.abilities_flying();
        if p.living.jumping && affected_by_fluids {
            let e = &p.entity;
            let fluid_height = if e.is_in_lava() { e.fluid_height(&fluid_tags::LAVA) } else { e.fluid_height(&fluid_tags::WATER) };
            let in_water_and_has_fluid_height = e.is_in_water() && fluid_height > 0.0;
            let threshold = e.fluid_jump_threshold();
            if !in_water_and_has_fluid_height || e.on_ground && !(fluid_height > threshold) {
                if !e.is_in_lava() || e.on_ground && e.fluid_height(&fluid_tags::LAVA) <= threshold {
                    if (e.on_ground || in_water_and_has_fluid_height && fluid_height <= threshold) && p.living.no_jump_delay == 0 {
                        self.jump_from_ground(id);
                        self.player_mut(id).living.no_jump_delay = 10;
                    }
                } else {
                    self.jump_in_liquid(id);
                }
            } else {
                self.jump_in_liquid(id);
            }
        } else {
            p.living.no_jump_delay = 0;
        }
        let l = &self.player(id).living;
        let input = Vec3::new(l.xxa as f64, l.yya as f64, l.zza as f64);
        if l.has_effect(mob_effects::SLOW_FALLING) || l.has_effect(mob_effects::LEVITATION) {
            self.reset_fall_distance(id);
        }
        self.player_travel(id, input);
        self.apply_effects_from_blocks(id);
        let e = self.entity_mut(id);
        if !e.is_in_powder_snow || !self.can_freeze(id) {
            let e = self.entity_mut(id);
            let ticks = (e.ticks_frozen() - 2).max(0);
            e.set_ticks_frozen(ticks);
        }
        self.remove_frost(id);
        self.try_add_frost(id);
        self.tick_freezing_damage(id);
    }

    fn remove_frost(&mut self, id: EntityId) {
        if let Some(speed) = self.player_mut(id).living.attributes.instance(Attribute::MovementSpeed)
            && speed.has_modifier(SPEED_MODIFIER_POWDER_SNOW_ID)
        {
            speed.remove_modifier(SPEED_MODIFIER_POWDER_SNOW_ID);
        }
    }

    fn try_add_frost(&mut self, id: EntityId) {
        let on = self.on_pos_legacy(id);
        if self.block_state(on).is_air() {
            return;
        }
        let ticks_frozen = self.entity(id).ticks_frozen();
        if ticks_frozen > 0 {
            let Some(speed) = self.player_mut(id).living.attributes.instance(Attribute::MovementSpeed) else { return };
            let percent_frozen = ticks_frozen.min(TICKS_REQUIRED_TO_FREEZE) as f32 / TICKS_REQUIRED_TO_FREEZE as f32;
            let slow_amount = -0.05f32 * percent_frozen;
            speed.add_transient_modifier(AttributeModifier {
                id: std::borrow::Cow::Borrowed(SPEED_MODIFIER_POWDER_SNOW_ID),
                amount: slow_amount as f64,
                operation: crate::world::entity::attributes::Operation::AddValue,
            });
        }
    }

    fn jump_in_liquid(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.add(0.0, 0.04f32 as f64, 0.0));
    }

    pub fn jump_from_ground(&mut self, id: EntityId) {
        self.living_jump_from_ground(id);
        let exhaustion = if self.entity(id).is_sprinting() { crate::world::food::constants::EXHAUSTION_SPRINT_JUMP } else { crate::world::food::constants::EXHAUSTION_JUMP };
        self.cause_food_exhaustion(id, exhaustion);
    }

    fn jump_boost_power(&self, id: EntityId) -> f32 {
        match self.player(id).living.effect(mob_effects::JUMP_BOOST) {
            Some(jump_boost) => 0.1 * (jump_boost.amplifier as f32 + 1.0),
            None => 0.0,
        }
    }

    fn living_jump_from_ground(&mut self, id: EntityId) {
        let jump_power = self.attribute_value(id, Attribute::JumpStrength) as f32 * 1.0 * self.block_jump_factor(id) + self.jump_boost_power(id);
        if jump_power <= 1.0e-5 {
            return;
        }
        let e = self.entity_mut(id);
        let m = e.delta_movement;
        e.set_delta_movement(Vec3::new(m.x, mth::jmax(jump_power as f64, m.y), m.z));
        if e.is_sprinting() {
            let angle = e.y_rot * mth::DEG_TO_RAD;
            e.add_delta_movement(Vec3::new(-mth::sin(angle as f64) as f64 * 0.2, 0.0, mth::cos(angle as f64) as f64 * 0.2));
        }
        e.needs_sync = true;
    }

    fn player_travel(&mut self, id: EntityId, input: Vec3) {
        let e = self.entity(id);
        if e.is_swimming() {
            let look_y = e.look_angle().y;
            let multiplier = if look_y < -0.2 { 0.085 } else { 0.06 };
            let above = BlockPos::containing(e.x(), e.y() + 1.0 - 0.1, e.z());
            if look_y <= 0.0 || self.player(id).living.jumping || !self.fluid_state(above).is_empty() {
                let e = self.entity_mut(id);
                let m = e.delta_movement;
                e.set_delta_movement(m.add(0.0, (look_y - m.y) * multiplier, 0.0));
            }
        }
        if self.player(id).abilities_flying() {
            let original_y = self.entity(id).delta_movement.y;
            self.travel(id, input);
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.with(crisp_core::Axis::Y, original_y * 0.6));
        } else {
            self.travel(id, input);
        }
    }

    fn travel(&mut self, id: EntityId, input: Vec3) {
        let affected_by_fluids = !self.player(id).abilities_flying();
        if self.entity(id).is_in_liquid() && affected_by_fluids {
            self.travel_in_fluid(id, input);
        } else {
            self.travel_in_air(id, input);
        }
    }

    fn travel_in_air(&mut self, id: EntityId, input: Vec3) {
        let pos_below = self.block_pos_below_that_affects_my_movement(id);
        let block_friction = if self.entity(id).on_ground {
            let friction = self.block_state(pos_below).block().info().friction;
            compute_modified_friction(friction, self.attribute_value(id, Attribute::FrictionModifier) as f32)
        } else {
            1.0
        };
        let movement = self.handle_relative_friction_and_calculate_movement(id, input, block_friction);
        let movement_y = match self.player(id).living.effect(mob_effects::LEVITATION) {
            Some(levitation) => movement.y + (0.05 * (levitation.amplifier + 1) as f64 - movement.y) * 0.2,
            None => movement.y - self.effective_gravity(id),
        };
        let air_drag_modifier = self.attribute_value(id, Attribute::AirDragModifier) as f32;
        let air_drag = compute_modified_friction(0.91, air_drag_modifier);
        let friction = block_friction * air_drag;
        let vertical_friction = compute_modified_friction(0.98, air_drag_modifier);
        let e = self.entity_mut(id);
        e.set_delta_movement(Vec3::new(movement.x * friction as f64, movement_y * vertical_friction as f64, movement.z * friction as f64));
    }

    fn travel_in_fluid(&mut self, id: EntityId, input: Vec3) {
        let e = self.entity(id);
        let is_falling = e.delta_movement.y <= 0.0;
        let old_y = e.y();
        let base_gravity = self.effective_gravity(id);
        if self.entity(id).is_in_water() {
            self.travel_in_water(id, input, base_gravity, is_falling, old_y);
        } else {
            self.travel_in_lava(id, input, base_gravity, is_falling, old_y);
        }
    }

    fn travel_in_water(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        let e = self.entity(id);
        let mut slow_down = if e.is_sprinting() { 0.9f32 } else { 0.8 };
        let mut speed = 0.02f32;
        let mut water_walker = self.attribute_value(id, Attribute::WaterMovementEfficiency) as f32;
        if !e.on_ground {
            water_walker *= 0.5;
        }
        if water_walker > 0.0 {
            slow_down += (0.54600006 - slow_down) * water_walker;
            speed += (self.living_speed(id) - speed) * water_walker;
        }
        if self.player(id).living.has_effect(mob_effects::DOLPHINS_GRACE) {
            slow_down = 0.96;
        }
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let mut movement = self.entity(id).delta_movement;
        if self.entity(id).horizontal_collision && self.on_climbable(id) {
            movement = Vec3::new(movement.x, 0.2, movement.z);
        }
        let movement = movement.multiply(slow_down as f64, 0.8f32 as f64, slow_down as f64);
        let adjusted = self.fluid_falling_adjusted_movement(id, base_gravity, is_falling, movement);
        self.entity_mut(id).set_delta_movement(adjusted);
        self.jump_out_of_fluid(id, old_y);
    }

    fn travel_in_lava(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        self.entity_mut(id).move_relative(0.02, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity(id);
        if e.fluid_height(&fluid_tags::LAVA) <= e.fluid_jump_threshold() {
            let m = e.delta_movement.multiply(0.5, 0.8f32 as f64, 0.5);
            self.entity_mut(id).set_delta_movement(m);
            let adjusted = self.fluid_falling_adjusted_movement(id, base_gravity, is_falling, m);
            self.entity_mut(id).set_delta_movement(adjusted);
        } else {
            let m = e.delta_movement.scale(0.5);
            self.entity_mut(id).set_delta_movement(m);
        }
        if base_gravity != 0.0 {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -base_gravity / 4.0, 0.0));
        }
        self.jump_out_of_fluid(id, old_y);
    }

    fn jump_out_of_fluid(&mut self, id: EntityId, old_y: f64) {
        let e = self.entity(id);
        let m = e.delta_movement;
        if e.horizontal_collision && self.is_free(id, m.x, m.y + 0.6f32 as f64 - self.entity(id).y() + old_y, m.z) {
            self.entity_mut(id).set_delta_movement(Vec3::new(m.x, 0.3f32 as f64, m.z));
        }
    }

    fn fluid_falling_adjusted_movement(&self, id: EntityId, base_gravity: f64, is_falling: bool, movement: Vec3) -> Vec3 {
        if base_gravity != 0.0 && !self.entity(id).is_sprinting() {
            let yd = if is_falling && (movement.y - 0.005).abs() >= 0.003 && (movement.y - base_gravity / 16.0).abs() < 0.003 {
                -0.003
            } else {
                movement.y - base_gravity / 16.0
            };
            Vec3::new(movement.x, yd, movement.z)
        } else {
            movement
        }
    }

    fn handle_relative_friction_and_calculate_movement(&mut self, id: EntityId, input: Vec3, friction: f32) -> Vec3 {
        let speed = self.friction_influenced_speed(id, friction);
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        let delta = self.handle_on_climbable(id, delta);
        self.entity_mut(id).set_delta_movement(delta);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity(id);
        let mut movement = e.delta_movement;
        let jumping = self.player(id).living.jumping;
        if (e.horizontal_collision || jumping) && (self.on_climbable(id) || self.entity(id).was_in_powder_snow && self.can_walk_on_powder_snow(id)) {
            movement = Vec3::new(movement.x, 0.2, movement.z);
        }
        movement
    }

    fn can_walk_on_powder_snow(&self, id: EntityId) -> bool {
        self.player(id).inventory.slots[8].as_ref().is_some_and(|s| s.item == items::LEATHER_BOOTS)
    }

    fn friction_influenced_speed(&self, id: EntityId, block_friction: f32) -> f32 {
        if self.entity(id).on_ground {
            let speed = self.living_speed(id);
            if block_friction as f64 > 0.6 { speed * (0.21600002 / (block_friction * block_friction * block_friction)) } else { speed }
        } else {
            self.flying_speed(id)
        }
    }

    fn flying_speed(&self, id: EntityId) -> f32 {
        let p = self.player(id);
        let sprinting = p.entity.is_sprinting();
        if p.abilities_flying() {
            let flying_speed = 0.05f32;
            if sprinting { flying_speed * 2.0 } else { flying_speed }
        } else if sprinting {
            0.025999999
        } else {
            0.02
        }
    }

    fn handle_on_climbable(&mut self, id: EntityId, delta: Vec3) -> Vec3 {
        if !self.on_climbable(id) {
            return delta;
        }
        self.reset_fall_distance(id);
        let max = 0.15f32 as f64;
        let xd = mth::clamp(delta.x, -max, max);
        let zd = mth::clamp(delta.z, -max, max);
        let mut yd = delta.y.max(-max);
        if yd < 0.0 && !self.in_block_state(id).is_block(blocks::SCAFFOLDING) && self.entity(id).is_shift_key_down() {
            yd = 0.0;
        }
        Vec3::new(xd, yd, zd)
    }

    pub fn on_climbable(&mut self, id: EntityId) -> bool {
        if self.is_spectator(id) {
            return false;
        }
        let pos = self.entity(id).block_position();
        let state = self.in_block_state(id);
        let climbable = state.is(&block_tags::CLIMBABLE)
            || state.block().is_instance_of("TrapDoorBlock") && state.get(OPEN) && {
                let below = self.block_state(pos.below());
                below.is_block(blocks::LADDER) && below.get(HORIZONTAL_FACING) == state.get(HORIZONTAL_FACING)
            };
        if climbable {
            self.player_mut(id).living.last_climbable_pos = Some(pos);
        }
        climbable
    }

    fn update_player_pose(&mut self, id: EntityId) {
        if !self.can_player_fit_when(id, Pose::Swimming) {
            return;
        }
        let p = self.player(id);
        let e = &p.entity;
        let desired = if e.is_swimming() {
            Pose::Swimming
        } else if e.is_fall_flying() {
            Pose::FallFlying
        } else if e.is_shift_key_down() && !p.abilities_flying() {
            Pose::Crouching
        } else {
            Pose::Standing
        };
        let actual = if self.is_spectator(id) || self.can_player_fit_when(id, desired) {
            desired
        } else if self.can_player_fit_when(id, Pose::Crouching) {
            Pose::Crouching
        } else {
            Pose::Swimming
        };
        self.set_player_pose(id, actual);
    }

    pub fn set_player_pose(&mut self, id: EntityId, pose: Pose) {
        let e = self.entity_mut(id);
        if e.pose() != pose {
            e.set_pose(pose);
            e.dimensions = player_dimensions(pose);
            e.reapply_position();
        }
    }

    fn can_player_fit_when(&self, id: EntityId, pose: Pose) -> bool {
        let box_ = player_dimensions(pose).make_bounding_box(self.entity(id).position()).deflate_all(1.0e-7);
        self.no_collision_for(id, &box_)
    }
}
