use crisp_core::math::OctahedralGroup;
use crisp_core::{Axis, Direction};

use super::rotation::Rotation;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mirror {
    None,
    LeftRight,
    FrontBack,
}

impl Mirror {
    pub fn mirror_index(self, rotation: i32, steps: i32) -> i32 {
        let half_steps = steps / 2;
        let corrected_rotation = if rotation > half_steps { rotation - steps } else { rotation };
        match self {
            Mirror::LeftRight => (half_steps - corrected_rotation + steps) % steps,
            Mirror::FrontBack => (steps - corrected_rotation) % steps,
            Mirror::None => rotation,
        }
    }

    pub fn get_rotation(self, value: Direction) -> Rotation {
        let axis = value.axis();
        if (self != Mirror::LeftRight || axis != Axis::Z) && (self != Mirror::FrontBack || axis != Axis::X) { Rotation::None } else { Rotation::Clockwise180 }
    }

    pub fn mirror(self, direction: Direction) -> Direction {
        if self == Mirror::FrontBack && direction.axis() == Axis::X || self == Mirror::LeftRight && direction.axis() == Axis::Z {
            direction.opposite()
        } else {
            direction
        }
    }

    pub fn rotation(self) -> OctahedralGroup {
        match self {
            Mirror::None => OctahedralGroup::Identity,
            Mirror::LeftRight => OctahedralGroup::InvertZ,
            Mirror::FrontBack => OctahedralGroup::InvertX,
        }
    }
}
