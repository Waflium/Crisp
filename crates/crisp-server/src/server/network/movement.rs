use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{Vec3, mth};
use crisp_data::packets::play::clientbound;

use crate::nbt::java_double_to_string;
use crate::server::level::ServerLevel;
use crate::server::level::game_mode::GameType;
use crate::server::level::server_player::ServerPlayer;
use crate::world::entity::relative::PositionMoveRotation;
use crate::world::entity::{EntityId, MoverType};

const CLIENT_LOADED_TIMEOUT: i32 = 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Input {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}

impl Input {
    pub fn from_flags(flags: u8) -> Input {
        Input {
            forward: flags & 1 != 0,
            backward: flags & 2 != 0,
            left: flags & 4 != 0,
            right: flags & 8 != 0,
            jump: flags & 16 != 0,
            shift: flags & 32 != 0,
            sprint: flags & 64 != 0,
        }
    }
}

mod player_command {
    pub const STOP_SLEEPING: i32 = 0;
    pub const START_SPRINTING: i32 = 1;
    pub const STOP_SPRINTING: i32 = 2;
}

pub struct PlayerMovement {
    pub tick_count: i32,
    pub first_good: Vec3,
    pub last_good: Vec3,
    pub awaiting_position_from_client: Option<Vec3>,
    pub awaiting_teleport: i32,
    pub awaiting_teleport_time: i32,
    pub client_is_floating: bool,
    pub above_ground_tick_count: i32,
    pub received_move_packet_count: i32,
    pub known_move_packet_count: i32,
    pub received_movement_this_tick: bool,
    pub received_position_this_tick: bool,
    pub client_loaded_timeout_timer: i32,
    pub waiting_for_respawn: bool,
    pub last_supported_blocks_send: i32,
}

impl Default for PlayerMovement {
    fn default() -> PlayerMovement {
        PlayerMovement {
            tick_count: 0,
            first_good: Vec3::ZERO,
            last_good: Vec3::ZERO,
            awaiting_position_from_client: None,
            awaiting_teleport: 0,
            awaiting_teleport_time: 0,
            client_is_floating: false,
            above_ground_tick_count: 0,
            received_move_packet_count: 0,
            known_move_packet_count: 0,
            received_movement_this_tick: false,
            received_position_this_tick: false,
            client_loaded_timeout_timer: CLIENT_LOADED_TIMEOUT,
            waiting_for_respawn: false,
            last_supported_blocks_send: 0,
        }
    }
}

impl PlayerMovement {
    pub fn has_client_loaded(&self) -> bool {
        !self.waiting_for_respawn && self.client_loaded_timeout_timer <= 0
    }

    pub fn tick_client_load_timeout(&mut self) {
        if self.client_loaded_timeout_timer > 0 {
            self.client_loaded_timeout_timer -= 1;
        }
    }
}

fn contains_invalid_values(x: f64, y: f64, z: f64, y_rot: f32, x_rot: f32) -> bool {
    x.is_nan() || y.is_nan() || z.is_nan() || !x_rot.is_finite() || !y_rot.is_finite()
}

fn clamp_horizontal(value: f64) -> f64 {
    mth::clamp(value, -3.0e7, 3.0e7)
}

fn clamp_vertical(value: f64) -> f64 {
    mth::clamp(value, -2.0e7, 2.0e7)
}

impl ServerPlayer {
    pub fn teleport(&mut self, pos: Vec3, y_rot: f32, x_rot: f32) {
        self.teleport_relative(PositionMoveRotation { position: pos, delta_movement: Vec3::ZERO, y_rot, x_rot }, 0);
    }

    pub fn teleport_relative(&mut self, destination: PositionMoveRotation, relatives: u16) {
        let id = self.teleport_set_position(destination, relatives);
        self.send_teleport(id, destination, relatives);
    }

    pub fn teleport_set_position(&mut self, destination: PositionMoveRotation, relatives: u16) -> i32 {
        let m = &mut self.movement;
        m.awaiting_teleport_time = m.tick_count;
        m.awaiting_teleport += 1;
        if m.awaiting_teleport == i32::MAX {
            m.awaiting_teleport = 0;
        }
        let id = m.awaiting_teleport;
        let e = &self.entity;
        let current = PositionMoveRotation { position: e.position(), delta_movement: self.known_movement, y_rot: e.y_rot, x_rot: e.x_rot };
        let e = &mut self.entity;
        e.teleport_set_position(current, destination, relatives);
        e.y_head_rot = e.y_rot;
        self.movement.awaiting_position_from_client = Some(self.entity.position());
        id
    }

    pub fn send_teleport(&self, id: i32, destination: PositionMoveRotation, relatives: u16) {
        self.connection.send(clientbound::PLAYER_POSITION, |p| {
            p.varint(id);
            destination.write(p);
            p.i32(relatives as i32);
        });
    }

    pub fn reset_position(&mut self) {
        let pos = self.entity.position();
        self.movement.first_good = pos;
        self.movement.last_good = pos;
    }
}

impl ServerLevel {
    pub fn handle_move_player(&mut self, id: EntityId, pos: Option<Vec3>, rot: Option<(f32, f32)>, on_ground: bool, horizontal_collision: bool) {
        let p = self.player(id);
        let target = pos.unwrap_or(p.entity.position());
        let (y_rot, x_rot) = rot.unwrap_or((p.entity.y_rot, p.entity.x_rot));
        let (check_y_rot, check_x_rot) = rot.unwrap_or((0.0, 0.0));
        let check = pos.unwrap_or(Vec3::ZERO);
        if contains_invalid_values(check.x, check.y, check.z, check_y_rot, check_x_rot) {
            self.player_mut(id).disconnect("multiplayer.disconnect.invalid_player_movement");
            return;
        }
        if pos.is_some() {
            if self.player(id).movement.received_position_this_tick {
                self.player_mut(id).disconnect("multiplayer.disconnect.invalid_player_movement");
                return;
            }
            self.player_mut(id).movement.received_position_this_tick = true;
        }
        if self.player(id).movement.tick_count == 0 {
            self.player_mut(id).reset_position();
        }
        if !self.player(id).movement.has_client_loaded() {
            return;
        }
        if self.update_awaiting_teleport(id) {
            self.entity_mut(id).abs_snap_rotation_to(mth::wrap_degrees(y_rot), mth::wrap_degrees(x_rot));
        } else {
            self.handle_player_position_change(id, target, y_rot, x_rot, on_ground, horizontal_collision);
        }
    }

    fn should_check_player_movement(&self, id: EntityId, is_fall_flying: bool) -> bool {
        if self.player(id).is_changing_dimension {
            return false;
        }
        self.game_rules.get_bool(crisp_data::game_rule::PLAYER_MOVEMENT_CHECK)
            && (!is_fall_flying || self.game_rules.get_bool(crisp_data::game_rule::ELYTRA_MOVEMENT_CHECK))
    }

    fn handle_player_position_change(&mut self, id: EntityId, requested: Vec3, requested_y_rot: f32, requested_x_rot: f32, on_ground: bool, horizontal_collision: bool) {
        let (target_x, target_y, target_z) = (clamp_horizontal(requested.x), clamp_vertical(requested.y), clamp_horizontal(requested.z));
        let target_y_rot = mth::wrap_degrees(requested_y_rot);
        let target_x_rot = mth::wrap_degrees(requested_x_rot);
        let p = self.player(id);
        let start = p.entity.position();
        let first_good = p.movement.first_good;
        let (x_dist, y_dist, z_dist) = (target_x - first_good.x, target_y - first_good.y, target_z - first_good.z);
        let expected_dist = p.entity.delta_movement.length_sqr();
        let moved_dist = x_dist * x_dist + y_dist * y_dist + z_dist * z_dist;
        let is_fall_flying = p.entity.is_fall_flying();
        if self.is_sleeping(id) {
            if moved_dist > 1.0 {
                self.player_mut(id).teleport(start, target_y_rot, target_x_rot);
            }
            return;
        }
        if self.tick_rate_manager.runs_normally() {
            let m = &mut self.player_mut(id).movement;
            m.received_move_packet_count += 1;
            let mut delta_packets = m.received_move_packet_count - m.known_move_packet_count;
            if delta_packets > 5 {
                delta_packets = 1;
            }
            if self.should_check_player_movement(id, is_fall_flying) {
                let meters_per_tick = if is_fall_flying { 300.0f32 } else { 100.0 };
                if moved_dist - expected_dist > (meters_per_tick * delta_packets as f32) as f64 {
                    let p = self.player(id);
                    warn!("{} moved too quickly! {},{},{}", p.profile.name, java_double_to_string(x_dist), java_double_to_string(y_dist), java_double_to_string(z_dist));
                    let (y_rot, x_rot) = (p.entity.y_rot, p.entity.x_rot);
                    self.player_mut(id).teleport(start, y_rot, x_rot);
                    return;
                }
            }
        }
        let p = self.player(id);
        let old_aabb = p.entity.bounding_box();
        let last_good = p.movement.last_good;
        let (x_dist, y_dist, z_dist) = (target_x - last_good.x, target_y - last_good.y, target_z - last_good.z);
        let moved_upwards = y_dist > 0.0;
        if p.entity.on_ground && !on_ground && moved_upwards {
            self.jump_from_ground(id);
        }
        let player_stands_on_something = self.entity(id).vertical_collision_below;
        if on_ground && !player_stands_on_something {
            self.force_send_player_support_blocks(id);
        }
        self.move_entity(id, MoverType::Player, Vec3::new(x_dist, y_dist, z_dist));
        let oy_dist = y_dist;
        let e = self.entity(id);
        let x_dist = target_x - e.x();
        let y_dist = 0.0;
        let z_dist = target_z - e.z();
        let moved_dist = x_dist * x_dist + y_dist * y_dist + z_dist * z_dist;
        let p = self.player(id);
        let fail = !p.is_changing_dimension
            && moved_dist > 0.0625
            && p.game_mode.game_type != GameType::Creative
            && p.game_mode.game_type != GameType::Spectator
            && !p.living.is_in_post_impulse_grace_time();
        if fail {
            warn!("{} moved wrongly!", p.profile.name);
        }
        let no_physics = p.entity.no_physics;
        if no_physics || (!fail || !self.no_collision_for(id, &old_aabb)) && !self.is_entity_colliding_with_anything_new(id, &old_aabb, target_x, target_y, target_z) {
            let e = self.entity_mut(id);
            e.abs_snap_to(target_x, target_y, target_z);
            e.abs_snap_rotation_to(target_y_rot, target_x_rot);
            let floating = oy_dist >= -0.03125
                && !player_stands_on_something
                && !self.is_spectator(id)
                && !self.allow_flight
                && !self.player(id).abilities_may_fly()
                && !self.has_effect(id, crisp_data::mob_effects::LEVITATION)
                && !is_fall_flying
                && {
                    let box_ = self.entity(id).bounding_box().inflate_all(0.0625).expand_towards(0.0, -0.55, 0.0);
                    self.all_air_in(&box_)
                };
            self.player_mut(id).movement.client_is_floating = floating;
            self.move_player(id);
            let e = self.entity(id);
            let client_delta = Vec3::new(e.x() - start.x, e.y() - start.y, e.z() - start.z);
            self.set_on_ground_with_movement(id, on_ground, horizontal_collision, client_delta);
            self.do_check_fall_damage(id, client_delta.x, client_delta.y, client_delta.z, on_ground);
            self.handle_player_known_movement(id, client_delta);
            if moved_upwards {
                self.reset_fall_distance(id);
            }
            let in_liquid_landed = {
                let e = self.entity(id);
                e.delta_movement.y < 1.0e-5f32 as f64 && e.is_in_liquid()
            };
            if on_ground || in_liquid_landed || self.on_climbable(id) || self.is_spectator(id) || is_fall_flying {
                self.player_mut(id).living.try_reset_current_impulse_context();
            }
            let e = self.entity(id);
            let (dx, dy, dz) = (e.x() - start.x, e.y() - start.y, e.z() - start.z);
            self.check_movement_statistics(id, dx, dy, dz);
            let p = self.player_mut(id);
            p.movement.last_good = p.entity.position();
        } else {
            self.player_mut(id).teleport(start, target_y_rot, target_x_rot);
            let e = self.entity(id);
            let (dx, dy, dz) = (e.x() - start.x, e.y() - start.y, e.z() - start.z);
            self.do_check_fall_damage(id, dx, dy, dz, on_ground);
            self.entity_mut(id).remove_latest_movement_recording();
        }
    }

    fn handle_player_known_movement(&mut self, id: EntityId, movement: Vec3) {
        let p = self.player_mut(id);
        if movement.length_sqr() > 1.0e-5f32 as f64 {
            p.reset_last_action_time();
        }
        p.known_movement = movement;
        p.movement.received_movement_this_tick = true;
    }

    fn force_send_player_support_blocks(&mut self, id: EntityId) {
        use crate::world::level::BlockGetter;
        let p = self.player_mut(id);
        if p.movement.tick_count < p.movement.last_supported_blocks_send + 200 {
            return;
        }
        p.movement.last_supported_blocks_send = p.movement.tick_count;
        info!("Player {} standing on air - force-sending blocks below", p.profile.name);
        let b = p.entity.bounding_box();
        let just_below = b.min_y - 1.0e-5f32 as f64;
        let mut positions = Vec::new();
        for (x, z) in [(b.min_x, b.min_z), (b.max_x, b.min_z), (b.min_x, b.max_z), (b.max_x, b.max_z)] {
            let pos = crisp_core::BlockPos::containing(x, just_below, z);
            if !positions.contains(&pos) {
                positions.push(pos);
            }
        }
        for pos in positions {
            let state = self.block_state(pos);
            self.player(id).connection.send(clientbound::BLOCK_UPDATE, |w| {
                w.position(pos.x, pos.y, pos.z);
                w.varint(state.id() as i32);
            });
            if let Some(packet) = self.block_entity_update_packet(pos) {
                self.player(id).connection.send_raw(packet);
            }
        }
    }

    fn update_awaiting_teleport(&mut self, id: EntityId) -> bool {
        let p = self.player_mut(id);
        match p.movement.awaiting_position_from_client {
            Some(pos) => {
                if p.movement.tick_count - p.movement.awaiting_teleport_time > 20 {
                    p.movement.awaiting_teleport_time = p.movement.tick_count;
                    let (y_rot, x_rot) = (p.entity.y_rot, p.entity.x_rot);
                    p.teleport(pos, y_rot, x_rot);
                }
                true
            }
            None => {
                p.movement.awaiting_teleport_time = p.movement.tick_count;
                false
            }
        }
    }

    fn is_entity_colliding_with_anything_new(&self, id: EntityId, old_aabb: &crisp_core::AABB, x: f64, y: f64, z: f64) -> bool {
        let e = self.entity(id);
        let new_aabb = e.bounding_box().move_by(x - e.x(), y - e.y(), z - e.z());
        let deflate = 1.0e-5f32 as f64;
        let old_shape = VoxelShape::from_boxes(&[old_aabb.deflate_all(deflate)]);
        for (_, collider) in self.block_collisions(&new_aabb.deflate_all(deflate), &crate::world::phys::shapes::collision_context::CollisionContext::of(self, id), false) {
            let [ox, oy, oz] = collider.offset;
            if !join_is_not_empty(&collider.shape.moved(ox, oy, oz), &old_shape, BooleanOp::And) {
                return true;
            }
        }
        false
    }

    pub fn handle_accept_teleport(&mut self, id: EntityId, teleport_id: i32, pos: Vec3, y_rot: f32, x_rot: f32) {
        let p = self.player_mut(id);
        if teleport_id != p.movement.awaiting_teleport {
            return;
        }
        let Some(awaiting) = p.movement.awaiting_position_from_client.filter(|_| !contains_invalid_values(pos.x, pos.y, pos.z, y_rot, x_rot)) else {
            p.disconnect("multiplayer.disconnect.invalid_player_movement");
            return;
        };
        let (own_y_rot, own_x_rot) = (p.entity.y_rot, p.entity.x_rot);
        p.entity.abs_snap_to(awaiting.x, awaiting.y, awaiting.z);
        p.entity.abs_snap_rotation_to(own_y_rot, own_x_rot);
        p.movement.last_good = awaiting;
        p.is_changing_dimension = false;
        p.movement.awaiting_position_from_client = None;
        self.handle_player_position_change(id, pos, y_rot, x_rot, false, false);
    }

    pub fn handle_player_input(&mut self, id: EntityId, input: Input) {
        let p = self.player_mut(id);
        p.last_client_input = input;
        if p.movement.has_client_loaded() {
            p.reset_last_action_time();
            p.entity.set_shift_key_down(input.shift);
        }
    }

    pub fn handle_player_command(&mut self, id: EntityId, action: i32) {
        if !self.player(id).movement.has_client_loaded() {
            return;
        }
        self.player_mut(id).reset_last_action_time();
        match action {
            player_command::STOP_SLEEPING => {
                if self.is_sleeping(id) {
                    self.stop_sleep_in_bed(id, false, true);
                    let position = self.entity(id).position();
                    self.player_mut(id).movement.awaiting_position_from_client = Some(position);
                }
            }
            player_command::START_SPRINTING => self.set_sprinting(id, true),
            player_command::STOP_SPRINTING => self.set_sprinting(id, false),
            _ => {}
        }
    }

    pub fn handle_player_loaded(&mut self, id: EntityId) {
        self.player_mut(id).movement.client_loaded_timeout_timer = 0;
    }

    pub fn handle_client_tick_end(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        if !p.movement.received_movement_this_tick {
            p.known_movement = Vec3::ZERO;
        }
        p.movement.received_movement_this_tick = false;
        p.movement.received_position_this_tick = false;
    }

    pub fn tick_player(&mut self, id: EntityId) -> bool {
        self.tick_player_start(id);
        self.pending_portal_teleport = None;
        self.tick_player_end(id)
    }

    pub fn tick_player_start(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        p.reset_position();
        p.entity.xo = p.entity.position();
        self.server_player_do_tick(id);
    }

    pub fn tick_player_end(&mut self, id: EntityId) -> bool {
        let sleeping = self.is_sleeping(id);
        let p = self.player_mut(id);
        let first_good = p.movement.first_good;
        let (y_rot, x_rot) = (p.entity.y_rot, p.entity.x_rot);
        p.entity.abs_snap_to(first_good.x, first_good.y, first_good.z);
        p.entity.abs_snap_rotation_to(y_rot, x_rot);
        p.movement.tick_count += 1;
        p.movement.known_move_packet_count = p.movement.received_move_packet_count;
        if p.movement.client_is_floating && !sleeping && p.health > 0.0 {
            p.movement.above_ground_tick_count += 1;
            let maximum = self.maximum_flying_ticks(id);
            let p = self.player_mut(id);
            if p.movement.above_ground_tick_count > maximum {
                warn!("{} was kicked for floating too long!", p.profile.name);
                p.disconnect("multiplayer.disconnect.flying");
                return true;
            }
        } else {
            p.movement.client_is_floating = false;
            p.movement.above_ground_tick_count = 0;
        }
        false
    }

    fn maximum_flying_ticks(&self, id: EntityId) -> i32 {
        let gravity = self.gravity(id);
        if gravity < 1.0e-5f32 as f64 {
            return i32::MAX;
        }
        let modifier = 0.08 / gravity;
        mth::ceil(80.0 * modifier.max(1.0))
    }
}
