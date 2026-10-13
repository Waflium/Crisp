use crisp_core::mth;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

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

pub fn rotate_towards(from_angle: f32, to_angle: f32, max_rot: f32) -> f32 {
    let diff = mth::degrees_difference(from_angle, to_angle);
    from_angle + mth::clamp_f(diff, -max_rot, max_rot)
}

impl ServerLevel {
    pub fn set_look_at(&mut self, id: EntityId, x: f64, y: f64, z: f64) {
        let mob = self.mob_mut(id).expect("a mob");
        let (head_rot_speed, max_head_x_rot) = (mob.get_head_rot_speed() as f32, mob.get_max_head_x_rot() as f32);
        mob.ai.look_control.set_look_at(x, y, z, head_rot_speed, max_head_x_rot);
    }

    pub fn set_look_at_entity(&mut self, id: EntityId, target: EntityId, y_max_rot_speed: f32, x_max_rot_angle: f32) {
        let t = self.entity(target);
        let (x, y, z) = (t.x(), t.eye_y(), t.z());
        self.mob_mut(id).expect("a mob").ai.look_control.set_look_at(x, y, z, y_max_rot_speed, x_max_rot_angle);
    }

    pub fn look_control_tick(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a mob");
        let navigation_done = mob.ai.navigation.is_done();
        let max_head_y_rot = mob.max_head_y_rot() as f32;
        let look = &mut mob.ai.look_control;
        let e = &mut mob.entity;
        e.set_x_rot(0.0);
        if look.look_at_cooldown > 0 {
            look.look_at_cooldown -= 1;
            let xd = look.wanted_x - e.x();
            let zd = look.wanted_z - e.z();
            if (zd.abs() > 1.0e-5f32 as f64) || (xd.abs() > 1.0e-5f32 as f64) {
                let y_rot_d = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
                e.y_head_rot = rotate_towards(e.y_head_rot, y_rot_d, look.y_max_rot_speed);
            }
            let yd = look.wanted_y - e.eye_y();
            let sd = (xd * xd + zd * zd).sqrt();
            if (yd.abs() > 1.0e-5f32 as f64) || (sd.abs() > 1.0e-5f32 as f64) {
                let x_rot_d = (-(mth::atan2(yd, sd) * mth::RAD_TO_DEG as f64)) as f32;
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
}
