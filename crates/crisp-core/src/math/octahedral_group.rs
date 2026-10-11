use super::SymmetricGroup3;
use crate::{Axis, Direction};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OctahedralGroup {
    Identity,
    Rot180FaceXz,
    Rot90YNeg,
    Rot90YPos,
    InvertX,
    InvertZ,
}

impl OctahedralGroup {
    pub const fn permutation(self) -> SymmetricGroup3 {
        match self {
            OctahedralGroup::Rot90YNeg | OctahedralGroup::Rot90YPos => SymmetricGroup3::P321,
            _ => SymmetricGroup3::P123,
        }
    }

    pub const fn inverts(self, axis: Axis) -> bool {
        let (invert_x, invert_y, invert_z) = match self {
            OctahedralGroup::Identity => (false, false, false),
            OctahedralGroup::Rot180FaceXz => (true, false, true),
            OctahedralGroup::Rot90YNeg => (true, false, false),
            OctahedralGroup::Rot90YPos => (false, false, true),
            OctahedralGroup::InvertX => (true, false, false),
            OctahedralGroup::InvertZ => (false, false, true),
        };
        match axis {
            Axis::X => invert_x,
            Axis::Y => invert_y,
            Axis::Z => invert_z,
        }
    }

    pub const fn rotate(self, direction: Direction) -> Direction {
        let old_direction = direction.axis_direction();
        let new_axis = self.permutation().inverse().permute_axis(direction.axis());
        let new_direction = if self.inverts(new_axis) { old_direction.opposite() } else { old_direction };
        Direction::from_axis_and_direction(new_axis, new_direction)
    }
}
