use crisp_core::{BlockPos, mth};
use crisp_data::block_tags;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::level::BlockGetter;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::pathfinder::pathfinding_context::PathfindingContext;
use crate::world::level::pathfinder::walk_node_evaluator::get_path_type_static;

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

impl ServerLevel {
    pub fn move_control_tick(&mut self, id: EntityId) {
        let speed_attribute = self.attribute_value(id, Attribute::MovementSpeed);
        let mob = self.mob_mut(id).expect("a mob");
        match mob.ai.move_control.operation {
            Operation::Strafe => {
                let control = &mob.ai.move_control;
                let speed = speed_attribute as f32;
                let speed_modified = control.speed_modifier as f32 * speed;
                let mut xa = control.strafe_forwards;
                let mut za = control.strafe_right;
                let mut dist = mth::sqrt(xa * xa + za * za);
                if dist < 1.0 {
                    dist = 1.0;
                }
                dist = speed_modified / dist;
                xa *= dist;
                za *= dist;
                let sin = mth::sin((mob.entity.y_rot * (std::f64::consts::PI / 180.0) as f32) as f64);
                let cos = mth::cos((mob.entity.y_rot * (std::f64::consts::PI / 180.0) as f32) as f64);
                let dx = xa * cos - za * sin;
                let dz = za * cos + xa * sin;
                if !self.is_walkable(id, dx, dz) {
                    let control = &mut self.mob_mut(id).expect("a mob").ai.move_control;
                    control.strafe_forwards = 1.0;
                    control.strafe_right = 0.0;
                }
                let mob = self.mob_mut(id).expect("a mob");
                mob.set_speed(speed_modified);
                let control = &mut mob.ai.move_control;
                mob.living.zza = control.strafe_forwards;
                mob.living.xxa = control.strafe_right;
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
                let y_rot_d = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
                let y_rot = rotlerp(e.y_rot, y_rot_d, 90.0);
                mob.entity.set_y_rot(y_rot);
                let speed = (control.speed_modifier * speed_attribute) as f32;
                mob.set_speed(speed);
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
                mob.set_speed(speed);
                let e = self.entity(id);
                if e.on_ground || e.is_in_liquid() {
                    self.mob_mut(id).expect("a mob").ai.move_control.operation = Operation::Wait;
                }
            }
            Operation::Wait => mob.living.zza = 0.0,
        }
    }

    fn is_walkable(&self, id: EntityId, dx: f32, dz: f32) -> bool {
        let e = self.entity(id);
        let pos = BlockPos::containing(e.x() + dx as f64, e.block_position().y as f64, e.z() + dz as f64);
        let context = PathfindingContext::new(PathNavigationRegion::new(self), id);
        get_path_type_static(&context, pos.x, pos.y, pos.z) == PathType::Walkable
    }
}
