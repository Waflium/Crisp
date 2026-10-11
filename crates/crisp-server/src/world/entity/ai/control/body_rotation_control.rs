use crisp_core::mth;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct BodyRotationControl {
    pub head_stable_time: i32,
    pub last_stable_y_head_rot: f32,
}

impl ServerLevel {
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
}
