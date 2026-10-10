use crisp_core::mth;
use crisp_data::block_tags;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::level::BlockGetter;

pub fn rotate_towards(from_angle: f32, to_angle: f32, max_rot: f32) -> f32 {
    let diff = mth::degrees_difference(from_angle, to_angle);
    from_angle + mth::clamp_f(diff, -max_rot, max_rot)
}

#[derive(Default)]
pub struct LookControl {
    pub y_max_rot_speed: f32,
    pub x_max_rot_angle: f32,
    pub look_at_cooldown: i32,
    pub wanted_x: f64,
    pub wanted_y: f64,
    pub wanted_z: f64,
}

impl LookControl {
    pub fn set_look_at(&mut self, x: f64, y: f64, z: f64, y_max_rot_speed: f32, x_max_rot_angle: f32) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.y_max_rot_speed = y_max_rot_speed;
        self.x_max_rot_angle = x_max_rot_angle;
        self.look_at_cooldown = 2;
    }

    pub fn is_looking_at_target(&self) -> bool {
        self.look_at_cooldown > 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Operation {
    #[default]
    Wait,
    MoveTo,
    Strafe,
    Jumping,
}

#[derive(Default)]
pub struct MoveControl {
    pub wanted_x: f64,
    pub wanted_y: f64,
    pub wanted_z: f64,
    pub speed_modifier: f64,
    pub strafe_forwards: f32,
    pub strafe_right: f32,
    pub operation: Operation,
}

impl MoveControl {
    pub fn has_wanted(&self) -> bool {
        self.operation == Operation::MoveTo
    }

    pub fn set_wanted_position(&mut self, x: f64, y: f64, z: f64, speed_modifier: f64) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.speed_modifier = speed_modifier;
        if self.operation != Operation::Jumping {
            self.operation = Operation::MoveTo;
        }
    }

    pub fn strafe(&mut self, forwards: f32, right: f32) {
        self.operation = Operation::Strafe;
        self.strafe_forwards = forwards;
        self.strafe_right = right;
        self.speed_modifier = 0.25;
    }

    pub fn set_wait(&mut self) {
        self.operation = Operation::Wait;
    }
}

fn rotlerp(a: f32, b: f32, max: f32) -> f32 {
    let diff = mth::clamp_f(mth::wrap_degrees(b - a), -max, max);
    let result = a + diff;
    if result < 0.0 {
        result + 360.0
    } else if result > 360.0 {
        result - 360.0
    } else {
        result
    }
}

#[derive(Default)]
pub struct JumpControl {
    pub jump: bool,
}

#[derive(Default)]
pub struct BodyRotationControl {
    pub head_stable_time: i32,
    pub last_stable_y_head_rot: f32,
}

impl ServerLevel {
    pub fn set_mob_speed(&mut self, id: EntityId, speed: f32) {
        let living = self.living_mut(id);
        living.speed = speed;
        living.zza = speed;
    }

    pub fn look_control_tick(&mut self, id: EntityId) {
        let navigation_done = self.navigation_is_done(id);
        let mob = self.mob_mut(id).expect("a mob");
        let max_head_y_rot = mob.max_head_y_rot() as f32;
        let look = &mut mob.ai.look_control;
        let e = &mut mob.entity;
        e.set_x_rot(0.0);
        if look.look_at_cooldown > 0 {
            look.look_at_cooldown -= 1;
            let xd = look.wanted_x - e.x();
            let zd = look.wanted_z - e.z();
            if (zd.abs() > 1.0e-5f32 as f64) || (xd.abs() > 1.0e-5f32 as f64) {
                let y_rot_d = (mth::atan2(zd, xd) * 180.0f32 as f64 / std::f32::consts::PI as f64) as f32 - 90.0;
                e.y_head_rot = rotate_towards(e.y_head_rot, y_rot_d, look.y_max_rot_speed);
            }
            let yd = look.wanted_y - e.eye_y();
            let sd = (xd * xd + zd * zd).sqrt();
            if (yd.abs() > 1.0e-5f32 as f64) || (sd.abs() > 1.0e-5f32 as f64) {
                let x_rot_d = (-(mth::atan2(yd, sd) * 180.0f32 as f64 / std::f32::consts::PI as f64)) as f32;
                let x_rot = rotate_towards(e.x_rot, x_rot_d, look.x_max_rot_angle);
                e.set_x_rot(x_rot);
            }
        } else {
            e.y_head_rot = rotate_towards(e.y_head_rot, mob.living.y_body_rot, 10.0);
        }
        if !navigation_done {
            e.y_head_rot = mth::rotate_if_necessary(e.y_head_rot, mob.living.y_body_rot, max_head_y_rot);
        }
    }

    pub fn move_control_tick(&mut self, id: EntityId) {
        let speed_attribute = self.attribute_value(id, Attribute::MovementSpeed);
        let mob = self.mob_mut(id).expect("a mob");
        match mob.ai.move_control.operation {
            Operation::Strafe => {
                let control = &mut mob.ai.move_control;
                let speed = speed_attribute as f32;
                let speed_modified = control.speed_modifier as f32 * speed;
                let living = &mut mob.living;
                living.speed = speed_modified;
                living.zza = control.strafe_forwards;
                living.xxa = control.strafe_right;
                control.operation = Operation::Wait;
            }
            Operation::MoveTo => {
                let control = &mut mob.ai.move_control;
                control.operation = Operation::Wait;
                let e = &mob.entity;
                let xd = control.wanted_x - e.x();
                let zd = control.wanted_z - e.z();
                let yd = control.wanted_y - e.y();
                let dd = xd * xd + yd * yd + zd * zd;
                if dd < 2.5000003e-7f32 as f64 {
                    mob.living.zza = 0.0;
                    return;
                }
                let y_rot_d = (mth::atan2(zd, xd) * 180.0f32 as f64 / std::f32::consts::PI as f64) as f32 - 90.0;
                let y_rot = rotlerp(e.y_rot, y_rot_d, 90.0);
                mob.entity.set_y_rot(y_rot);
                let speed = (control.speed_modifier * speed_attribute) as f32;
                self.set_mob_speed(id, speed);
                let pos = self.entity(id).block_position();
                let state = self.block_state(pos);
                let shape = state.collision_voxel_shape();
                let e = self.entity(id);
                let max_up_step = self.max_up_step(id) as f64;
                let width = e.dimensions.width;
                let blocks_jump = !shape.is_empty()
                    && e.y() < shape.max(crisp_core::Axis::Y) + pos.y as f64
                    && !state.is(&block_tags::DOORS)
                    && !state.is(&block_tags::FENCES);
                if yd > max_up_step && xd * xd + zd * zd < 1.0f32.max(width) as f64 || blocks_jump {
                    let mob = self.mob_mut(id).expect("a mob");
                    mob.ai.jump_control.jump = true;
                    mob.ai.move_control.operation = Operation::Jumping;
                }
            }
            Operation::Jumping => {
                let speed = (mob.ai.move_control.speed_modifier * speed_attribute) as f32;
                self.set_mob_speed(id, speed);
                let e = self.entity(id);
                if e.on_ground || e.is_in_liquid() {
                    self.mob_mut(id).expect("a mob").ai.move_control.operation = Operation::Wait;
                }
            }
            Operation::Wait => mob.living.zza = 0.0,
        }
    }

    pub fn jump_control_tick(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a mob");
        mob.living.jumping = mob.ai.jump_control.jump;
        mob.ai.jump_control.jump = false;
    }

    pub fn body_rotation_control_tick(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a mob");
        let max_head_y_rot = mob.max_head_y_rot() as f32;
        let e = &mut mob.entity;
        let control = &mut mob.ai.body_rotation_control;
        let xd = e.x() - e.xo.x;
        let zd = e.z() - e.xo.z;
        if xd * xd + zd * zd > 2.5000003e-7f32 as f64 {
            mob.living.y_body_rot = e.y_rot;
            e.y_head_rot = mth::rotate_if_necessary(e.y_head_rot, mob.living.y_body_rot, max_head_y_rot);
            control.last_stable_y_head_rot = e.y_head_rot;
            control.head_stable_time = 0;
        } else if (e.y_head_rot - control.last_stable_y_head_rot).abs() > 15.0 {
            control.head_stable_time = 0;
            control.last_stable_y_head_rot = e.y_head_rot;
            mob.living.y_body_rot = mth::rotate_if_necessary(mob.living.y_body_rot, e.y_head_rot, max_head_y_rot);
        } else {
            control.head_stable_time += 1;
            if control.head_stable_time > 10 {
                let time_since_starting_to_face_forward = control.head_stable_time - 10;
                let face_forward_fraction = mth::clamp_f(time_since_starting_to_face_forward as f32 / 10.0, 0.0, 1.0);
                let angle_remaining = max_head_y_rot * (1.0 - face_forward_fraction);
                mob.living.y_body_rot = mth::rotate_if_necessary(mob.living.y_body_rot, e.y_head_rot, angle_remaining);
            }
        }
    }

    pub fn navigation_is_done(&self, _id: EntityId) -> bool {
        true
    }
}
