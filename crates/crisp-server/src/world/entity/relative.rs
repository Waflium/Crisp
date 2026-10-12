use crisp_core::{Vec3, mth};
use crisp_protocol::Writer;

pub mod relative {
    pub const X: u16 = 1 << 0;
    pub const Y: u16 = 1 << 1;
    pub const Z: u16 = 1 << 2;
    pub const Y_ROT: u16 = 1 << 3;
    pub const X_ROT: u16 = 1 << 4;
    pub const DELTA_X: u16 = 1 << 5;
    pub const DELTA_Y: u16 = 1 << 6;
    pub const DELTA_Z: u16 = 1 << 7;
    pub const ROTATE_DELTA: u16 = 1 << 8;
    pub const ROTATION: u16 = X_ROT | Y_ROT;
    pub const DELTA: u16 = DELTA_X | DELTA_Y | DELTA_Z | ROTATE_DELTA;
    pub const ALL: u16 = X | Y | Z | ROTATION | DELTA_X | DELTA_Y | DELTA_Z | ROTATE_DELTA;

    pub fn rotation(relative_y_rot: bool, relative_x_rot: bool) -> u16 {
        (if relative_y_rot { Y_ROT } else { 0 }) | if relative_x_rot { X_ROT } else { 0 }
    }

    pub fn position(x: bool, y: bool, z: bool) -> u16 {
        (if x { X } else { 0 }) | (if y { Y } else { 0 }) | if z { Z } else { 0 }
    }

    pub fn direction(x: bool, y: bool, z: bool) -> u16 {
        (if x { DELTA_X } else { 0 }) | (if y { DELTA_Y } else { 0 }) | if z { DELTA_Z } else { 0 }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PositionMoveRotation {
    pub position: Vec3,
    pub delta_movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
}

impl PositionMoveRotation {
    pub fn write(&self, w: &mut Writer) {
        for v in [self.position, self.delta_movement] {
            w.f64(v.x);
            w.f64(v.y);
            w.f64(v.z);
        }
        w.f32(self.y_rot);
        w.f32(self.x_rot);
    }

    pub fn calculate_absolute(source: PositionMoveRotation, change: PositionMoveRotation, relatives: u16) -> PositionMoveRotation {
        let has = |r: u16| relatives & r != 0;
        let offset_x = if has(relative::X) { source.position.x } else { 0.0 };
        let offset_y = if has(relative::Y) { source.position.y } else { 0.0 };
        let offset_z = if has(relative::Z) { source.position.z } else { 0.0 };
        let offset_y_rot = if has(relative::Y_ROT) { source.y_rot } else { 0.0 };
        let offset_x_rot = if has(relative::X_ROT) { source.x_rot } else { 0.0 };
        let position = Vec3::new(offset_x + change.position.x, offset_y + change.position.y, offset_z + change.position.z);
        let y_rot = offset_y_rot + change.y_rot;
        let x_rot = mth::clamp_f(offset_x_rot + change.x_rot, -90.0, 90.0);
        let mut rotated = source.delta_movement;
        if has(relative::ROTATE_DELTA) {
            let diff_y_rot = source.y_rot - y_rot;
            let diff_x_rot = source.x_rot - x_rot;
            rotated = rotated.x_rot((diff_x_rot as f64).to_radians() as f32);
            rotated = rotated.y_rot((diff_y_rot as f64).to_radians() as f32);
        }
        let delta = |current: f64, change: f64, r: u16| if has(r) { current + change } else { change };
        let delta_movement = Vec3::new(
            delta(rotated.x, change.delta_movement.x, relative::DELTA_X),
            delta(rotated.y, change.delta_movement.y, relative::DELTA_Y),
            delta(rotated.z, change.delta_movement.z, relative::DELTA_Z),
        );
        PositionMoveRotation { position, delta_movement, y_rot, x_rot }
    }
}
