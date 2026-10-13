use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::properties::{POWERED, RailShape};
use crisp_data::{block_tags, blocks, registry, registry_id};

use super::abstract_minecart::{exits, rail_shape};
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, MoverType};
use crate::world::level::BlockGetter;
use crate::world::level::block::base_rail_block;

const MAX_SPEED_IN_WATER: f64 = 0.2;
const MAX_SPEED_ON_LAND: f64 = 0.4;

#[derive(Default)]
pub struct OldMinecartBehavior {
    target_delta_movement: Vec3,
}

impl OldMinecartBehavior {
    pub fn lerp_motion(&mut self, movement: Vec3) -> Vec3 {
        self.target_delta_movement = movement;
        self.target_delta_movement
    }
}

impl ServerLevel {
    pub fn old_minecart_behavior_tick(&mut self, id: EntityId) {
        self.apply_gravity(id);
        let pos = self.get_current_block_pos_or_rail_below(id);
        let state = self.block_state(pos);
        let on_rails = base_rail_block::is_rail(state);
        self.minecart_mut(id).set_on_rails(on_rails);
        if on_rails {
            self.old_minecart_behavior_move_along_track(id);
            if self.entity_ref(id).is_none_or(|e| e.is_removed()) {
                return;
            }
            if state.is_block(blocks::ACTIVATOR_RAIL) {
                self.activate_minecart(id, pos.x, pos.y, pos.z, state.get(POWERED));
            }
        } else {
            self.come_off_track(id);
        }
        if self.entity_ref(id).is_none_or(|e| e.is_removed()) {
            return;
        }
        self.minecart_apply_effects_from_blocks(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        let flipped = self.minecart(id).is_flipped();
        let e = self.entity_mut(id);
        e.set_x_rot(0.0);
        let x_diff = e.xo.x - e.x();
        let z_diff = e.xo.z - e.z();
        if x_diff * x_diff + z_diff * z_diff > 0.001 {
            e.set_y_rot((mth::atan2(z_diff, x_diff) * 180.0 / std::f64::consts::PI) as f32);
            if flipped {
                e.set_y_rot(e.y_rot + 180.0);
            }
        }
        let rot_diff = mth::wrap_degrees(e.y_rot - e.y_rot_o) as f64;
        if rot_diff < -170.0 || rot_diff >= 170.0 {
            e.set_y_rot(e.y_rot + 180.0);
            self.minecart_mut(id).set_flipped(!flipped);
        }
        let e = self.entity_mut(id);
        e.set_x_rot(e.x_rot % 360.0);
        e.set_y_rot(e.y_rot % 360.0);
        self.old_minecart_behavior_push_and_pickup_entities(id);
    }

    pub fn old_minecart_behavior_move_along_track(&mut self, id: EntityId) {
        let pos = self.get_current_block_pos_or_rail_below(id);
        let state = self.block_state(pos);
        self.reset_fall_distance(id);
        let e = self.entity(id);
        let (x, z) = (e.x(), e.z());
        let old_pos = self.old_minecart_behavior_get_pos(x, e.y(), z);
        let mut y = pos.y as f64;
        let mut power_track = false;
        let mut halt_track = false;
        if state.is_block(blocks::POWERED_RAIL) {
            power_track = state.get(POWERED);
            halt_track = !power_track;
        }
        let mut slide_speed = 0.0078125;
        if self.entity(id).is_in_water() {
            slide_speed *= 0.2;
        }
        let shape = rail_shape(state);
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        match shape {
            RailShape::AscendingEast => {
                e.set_delta_movement(movement.add(-slide_speed, 0.0, 0.0));
                y += 1.0;
            }
            RailShape::AscendingWest => {
                e.set_delta_movement(movement.add(slide_speed, 0.0, 0.0));
                y += 1.0;
            }
            RailShape::AscendingNorth => {
                e.set_delta_movement(movement.add(0.0, 0.0, slide_speed));
                y += 1.0;
            }
            RailShape::AscendingSouth => {
                e.set_delta_movement(movement.add(0.0, 0.0, -slide_speed));
                y += 1.0;
            }
            _ => {}
        }
        let movement = e.delta_movement;
        let (exit0, exit1) = exits(shape);
        let mut x_d = (exit1[0] - exit0[0]) as f64;
        let mut z_d = (exit1[2] - exit0[2]) as f64;
        let length = (x_d * x_d + z_d * z_d).sqrt();
        let flip = movement.x * x_d + movement.z * z_d;
        if flip < 0.0 {
            x_d = -x_d;
            z_d = -z_d;
        }
        let pow = movement.horizontal_distance().min(2.0);
        e.set_delta_movement(Vec3::new(pow * x_d / length, movement.y, pow * z_d / length));
        let controlling_passenger = self.first_passenger(id);
        let move_intent = match controlling_passenger.and_then(|p| self.players.get(&p)) {
            Some(player) => player.get_last_client_move_intent(),
            None => Vec3::ZERO,
        };
        if controlling_passenger.is_some_and(|p| self.is_player(p)) && move_intent.length_sqr() > 0.0 {
            let rider_movement = move_intent.normalize();
            let e = self.entity_mut(id);
            let own_dist = e.delta_movement.horizontal_distance_sqr();
            if rider_movement.length_sqr() > 0.0 && own_dist < 0.01 {
                e.set_delta_movement(e.delta_movement.add(move_intent.x * 0.001, 0.0, move_intent.z * 0.001));
                halt_track = false;
            }
        }
        let e = self.entity_mut(id);
        if halt_track {
            let speed_length = e.delta_movement.horizontal_distance();
            if speed_length < 0.03 {
                e.set_delta_movement(Vec3::ZERO);
            } else {
                e.set_delta_movement(e.delta_movement.multiply(0.5, 0.0, 0.5));
            }
        }
        let x0 = pos.x as f64 + 0.5 + exit0[0] as f64 * 0.5;
        let z0 = pos.z as f64 + 0.5 + exit0[2] as f64 * 0.5;
        let x1 = pos.x as f64 + 0.5 + exit1[0] as f64 * 0.5;
        let z1 = pos.z as f64 + 0.5 + exit1[2] as f64 * 0.5;
        let x_d = x1 - x0;
        let z_d = z1 - z0;
        let progress = if x_d == 0.0 {
            z - pos.z as f64
        } else if z_d == 0.0 {
            x - pos.x as f64
        } else {
            let xx = x - x0;
            let zz = z - z0;
            (xx * x_d + zz * z_d) * 2.0
        };
        let x = x0 + x_d * progress;
        let z = z0 + z_d * progress;
        e.set_pos(Vec3::new(x, y, z));
        self.entity_moved(id);
        let scale = if self.is_vehicle(id) { 0.75 } else { 1.0 };
        let max_speed = self.minecart_max_speed(id);
        let movement = self.entity(id).delta_movement;
        self.minecart_move(
            id,
            MoverType::SelfMove,
            Vec3::new(mth::clamp(scale * movement.x, -max_speed, max_speed), 0.0, mth::clamp(scale * movement.z, -max_speed, max_speed)),
        );
        if self.entity_ref(id).is_none_or(|e| e.is_removed()) {
            return;
        }
        let e = self.entity_mut(id);
        if exit0[1] != 0 && mth::floor(e.x()) - pos.x == exit0[0] && mth::floor(e.z()) - pos.z == exit0[2] {
            e.set_pos(Vec3::new(e.x(), e.y() + exit0[1] as f64, e.z()));
        } else if exit1[1] != 0 && mth::floor(e.x()) - pos.x == exit1[0] && mth::floor(e.z()) - pos.z == exit1[2] {
            e.set_pos(Vec3::new(e.x(), e.y() + exit1[1] as f64, e.z()));
        }
        let slowed = self.apply_natural_slowdown(id, self.entity(id).delta_movement);
        self.entity_mut(id).set_delta_movement(slowed);
        let e = self.entity(id);
        let new_pos = self.old_minecart_behavior_get_pos(e.x(), e.y(), e.z());
        if let (Some(new_pos), Some(old_pos)) = (new_pos, old_pos) {
            let speed = (old_pos.y - new_pos.y) * 0.05;
            let e = self.entity_mut(id);
            let vec3 = e.delta_movement;
            let other_pow = vec3.horizontal_distance();
            if other_pow > 0.0 {
                e.set_delta_movement(vec3.multiply((other_pow + speed) / other_pow, 1.0, (other_pow + speed) / other_pow));
            }
            e.set_pos(Vec3::new(e.x(), new_pos.y, e.z()));
        }
        let e = self.entity_mut(id);
        let xn = mth::floor(e.x());
        let zn = mth::floor(e.z());
        if xn != pos.x || zn != pos.z {
            let vec3 = e.delta_movement;
            let other_pow = vec3.horizontal_distance();
            e.set_delta_movement(Vec3::new(other_pow * (xn - pos.x) as f64, vec3.y, other_pow * (zn - pos.z) as f64));
        }
        self.entity_moved(id);
        if power_track {
            let vec3 = self.entity(id).delta_movement;
            let speed_length = vec3.horizontal_distance();
            if speed_length > 0.01 {
                self.entity_mut(id).set_delta_movement(vec3.add(vec3.x / speed_length * 0.06, 0.0, vec3.z / speed_length * 0.06));
            } else {
                let delta_movement = self.entity(id).delta_movement;
                let (mut dx, mut dz) = (delta_movement.x, delta_movement.z);
                if shape == RailShape::EastWest {
                    if self.is_redstone_conductor(pos.west()) {
                        dx = 0.02;
                    } else if self.is_redstone_conductor(pos.east()) {
                        dx = -0.02;
                    }
                } else {
                    if shape != RailShape::NorthSouth {
                        return;
                    }
                    if self.is_redstone_conductor(pos.north()) {
                        dz = 0.02;
                    } else if self.is_redstone_conductor(pos.south()) {
                        dz = -0.02;
                    }
                }
                self.entity_mut(id).set_delta_movement(Vec3::new(dx, delta_movement.y, dz));
            }
        }
    }

    pub fn old_minecart_behavior_get_pos(&mut self, x: f64, y: f64, z: f64) -> Option<Vec3> {
        let xt = mth::floor(x);
        let mut yt = mth::floor(y);
        let zt = mth::floor(z);
        if self.block_state(BlockPos::new(xt, yt - 1, zt)).is(&block_tags::RAILS) {
            yt -= 1;
        }
        let state = self.block_state(BlockPos::new(xt, yt, zt));
        if !base_rail_block::is_rail(state) {
            return None;
        }
        let shape = rail_shape(state);
        let (exit0, exit1) = exits(shape);
        let x0 = xt as f64 + 0.5 + exit0[0] as f64 * 0.5;
        let y0 = yt as f64 + 0.0625 + exit0[1] as f64 * 0.5;
        let z0 = zt as f64 + 0.5 + exit0[2] as f64 * 0.5;
        let x1 = xt as f64 + 0.5 + exit1[0] as f64 * 0.5;
        let y1 = yt as f64 + 0.0625 + exit1[1] as f64 * 0.5;
        let z1 = zt as f64 + 0.5 + exit1[2] as f64 * 0.5;
        let x_d = x1 - x0;
        let y_d = (y1 - y0) * 2.0;
        let z_d = z1 - z0;
        let progress = if x_d == 0.0 {
            z - zt as f64
        } else if z_d == 0.0 {
            x - xt as f64
        } else {
            let xx = x - x0;
            let zz = z - z0;
            (xx * x_d + zz * z_d) * 2.0
        };
        let x = x0 + x_d * progress;
        let mut y = y0 + y_d * progress;
        let z = z0 + z_d * progress;
        if y_d < 0.0 {
            y += 1.0;
        } else if y_d > 0.0 {
            y += 0.5;
        }
        Some(Vec3::new(x, y, z))
    }

    pub fn old_minecart_behavior_push_and_pickup_entities(&mut self, id: EntityId) -> bool {
        let hitbox = self.entity(id).bounding_box().inflate(0.2f32 as f64, 0.0, 0.2f32 as f64);
        if self.minecart(id).is_rideable() && self.entity(id).delta_movement.horizontal_distance_sqr() >= 0.01 {
            let entities: Vec<EntityId> = self.get_entities(Some(id), &hitbox).into_iter().filter(|&e| self.pushable_by(id, e)).collect();
            for entity in entities {
                let pickup = !self.is_player(entity)
                    && self.entity_ref(entity).is_some_and(|e| e.entity_type != registry_id(registry::ENTITY_TYPE, "minecraft:iron_golem"))
                    && !self.is_minecart(entity)
                    && !self.is_vehicle(id)
                    && !self.is_passenger(entity);
                if pickup {
                    self.start_riding(entity, id, false, true);
                } else {
                    self.push_entity(entity, id);
                }
            }
        } else {
            for entity in self.get_entities(Some(id), &hitbox) {
                if !self.has_passenger(id, entity) && self.is_pushable(entity) && self.is_minecart(entity) {
                    self.push_entity(entity, id);
                }
            }
        }
        false
    }

    pub fn old_minecart_behavior_max_speed(&self, id: EntityId) -> f64 {
        if self.entity(id).is_in_water() { MAX_SPEED_IN_WATER } else { MAX_SPEED_ON_LAND }
    }

    pub fn old_minecart_behavior_slowdown_factor(&self, id: EntityId) -> f64 {
        if self.is_vehicle(id) { 0.997 } else { 0.96 }
    }

    pub fn old_minecart_behavior_known_movement(known_movement: Vec3) -> Vec3 {
        if !known_movement.x.is_nan() && !known_movement.y.is_nan() && !known_movement.z.is_nan() {
            Vec3::new(mth::clamp(known_movement.x, -0.4, 0.4), known_movement.y, mth::clamp(known_movement.z, -0.4, 0.4))
        } else {
            Vec3::ZERO
        }
    }
}
