use crisp_core::math::OctahedralGroup;
use crisp_core::random::RandomSource;
use crisp_core::{Axis, Direction};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    Counterclockwise90,
}

impl Rotation {
    pub const VALUES: [Rotation; 4] = [Rotation::None, Rotation::Clockwise90, Rotation::Clockwise180, Rotation::Counterclockwise90];

    pub fn by_name(name: &str) -> Option<Rotation> {
        Some(match name {
            "none" => Rotation::None,
            "clockwise_90" => Rotation::Clockwise90,
            "180" => Rotation::Clockwise180,
            "counterclockwise_90" => Rotation::Counterclockwise90,
            _ => return None,
        })
    }

    pub fn rotation(self) -> OctahedralGroup {
        match self {
            Rotation::None => OctahedralGroup::Identity,
            Rotation::Clockwise90 => OctahedralGroup::Rot90YNeg,
            Rotation::Clockwise180 => OctahedralGroup::Rot180FaceXz,
            Rotation::Counterclockwise90 => OctahedralGroup::Rot90YPos,
        }
    }

    pub fn rotate(self, direction: Direction) -> Direction {
        if direction.axis() == Axis::Y {
            return direction;
        }
        match self {
            Rotation::Clockwise90 => direction.clockwise(),
            Rotation::Clockwise180 => direction.opposite(),
            Rotation::Counterclockwise90 => direction.counter_clockwise(),
            Rotation::None => direction,
        }
    }

    pub fn rotate_index(self, rotation: i32, steps: i32) -> i32 {
        match self {
            Rotation::Clockwise90 => (rotation + steps / 4) % steps,
            Rotation::Clockwise180 => (rotation + steps / 2) % steps,
            Rotation::Counterclockwise90 => (rotation + steps * 3 / 4) % steps,
            Rotation::None => rotation,
        }
    }

    pub fn get_random(random: &mut impl RandomSource) -> Rotation {
        Rotation::VALUES[random.next_int_bounded(4) as usize]
    }
}
