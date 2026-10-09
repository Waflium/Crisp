use crate::mth;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisDirection {
    Positive,
    Negative,
}

use Direction::*;

impl Direction {
    pub const VALUES: [Direction; 6] = [Down, Up, North, South, West, East];
    pub const HORIZONTAL: [Direction; 4] = [South, West, North, East];
    pub const PLANE_HORIZONTAL: [Direction; 4] = [North, East, South, West];

    pub const fn ordinal(self) -> usize {
        self as usize
    }

    pub const fn from_3d_data_value(data: i32) -> Direction {
        Self::VALUES[(data % 6).unsigned_abs() as usize]
    }

    pub const fn from_2d_data_value(data: i32) -> Direction {
        Self::HORIZONTAL[(data % 4).unsigned_abs() as usize]
    }

    pub const fn get_2d_data_value(self) -> i32 {
        match self {
            South => 0,
            West => 1,
            North => 2,
            East => 3,
            Down | Up => -1,
        }
    }

    pub const fn opposite(self) -> Direction {
        match self {
            Down => Up,
            Up => Down,
            North => South,
            South => North,
            West => East,
            East => West,
        }
    }

    pub const fn axis(self) -> Axis {
        match self {
            Down | Up => Axis::Y,
            North | South => Axis::Z,
            West | East => Axis::X,
        }
    }

    pub const fn axis_direction(self) -> AxisDirection {
        match self {
            Up | South | East => AxisDirection::Positive,
            Down | North | West => AxisDirection::Negative,
        }
    }

    pub const fn step_x(self) -> i32 {
        match self {
            West => -1,
            East => 1,
            _ => 0,
        }
    }

    pub const fn step_y(self) -> i32 {
        match self {
            Down => -1,
            Up => 1,
            _ => 0,
        }
    }

    pub const fn step_z(self) -> i32 {
        match self {
            North => -1,
            South => 1,
            _ => 0,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Down => "down",
            Up => "up",
            North => "north",
            South => "south",
            West => "west",
            East => "east",
        }
    }

    pub fn by_name(name: &str) -> Option<Direction> {
        Self::VALUES.into_iter().find(|d| d.name() == name)
    }

    pub fn clockwise(self) -> Direction {
        match self {
            North => East,
            South => West,
            West => North,
            East => South,
            _ => panic!("Unable to get Y-rotated facing of {self:?}"),
        }
    }

    pub fn counter_clockwise(self) -> Direction {
        match self {
            North => West,
            South => East,
            West => South,
            East => North,
            _ => panic!("Unable to get CCW facing of {self:?}"),
        }
    }

    pub fn clockwise_around(self, axis: Axis) -> Direction {
        match axis {
            Axis::X => match self {
                Down => South,
                Up => North,
                North => Down,
                South => Up,
                _ => self,
            },
            Axis::Y => match self {
                Up | Down => self,
                _ => self.clockwise(),
            },
            Axis::Z => match self {
                Down => West,
                Up => East,
                West => Up,
                East => Down,
                _ => self,
            },
        }
    }

    pub fn counter_clockwise_around(self, axis: Axis) -> Direction {
        match axis {
            Axis::X => match self {
                Down => North,
                Up => South,
                North => Up,
                South => Down,
                _ => self,
            },
            Axis::Y => match self {
                Up | Down => self,
                _ => self.counter_clockwise(),
            },
            Axis::Z => match self {
                Down => East,
                Up => West,
                West => Down,
                East => Up,
                _ => self,
            },
        }
    }

    pub fn from_y_rot(y_rot: f64) -> Direction {
        Self::from_2d_data_value(mth::floor(y_rot / 90.0 + 0.5) & 3)
    }

    pub fn to_y_rot(self) -> f32 {
        ((self.get_2d_data_value() & 3) * 90) as f32
    }

    pub fn y_rot(self) -> f32 {
        match self {
            North => 180.0,
            South => 0.0,
            West => 90.0,
            East => -90.0,
            _ => panic!("No y-Rot for vertical axis: {self:?}"),
        }
    }

    pub const fn from_axis_and_direction(axis: Axis, direction: AxisDirection) -> Direction {
        match (axis, direction) {
            (Axis::X, AxisDirection::Positive) => East,
            (Axis::X, AxisDirection::Negative) => West,
            (Axis::Y, AxisDirection::Positive) => Up,
            (Axis::Y, AxisDirection::Negative) => Down,
            (Axis::Z, AxisDirection::Positive) => South,
            (Axis::Z, AxisDirection::Negative) => North,
        }
    }

    pub fn ordered_by_nearest(x_rot: f32, y_rot: f32) -> [Direction; 6] {
        let pitch = x_rot * mth::DEG_TO_RAD;
        let yaw = -y_rot * mth::DEG_TO_RAD;
        let pitch_sin = mth::sin(pitch as f64);
        let pitch_cos = mth::cos(pitch as f64);
        let yaw_sin = mth::sin(yaw as f64);
        let yaw_cos = mth::cos(yaw as f64);
        let x_pos = yaw_sin > 0.0;
        let y_pos = pitch_sin < 0.0;
        let z_pos = yaw_cos > 0.0;
        let x_yaw = if x_pos { yaw_sin } else { -yaw_sin };
        let y_mag = if y_pos { -pitch_sin } else { pitch_sin };
        let z_yaw = if z_pos { yaw_cos } else { -yaw_cos };
        let x_mag = x_yaw * pitch_cos;
        let z_mag = z_yaw * pitch_cos;
        let axis_x = if x_pos { East } else { West };
        let axis_y = if y_pos { Up } else { Down };
        let axis_z = if z_pos { South } else { North };
        let make = |a: Direction, b: Direction, c: Direction| [a, b, c, c.opposite(), b.opposite(), a.opposite()];
        if x_yaw > z_yaw {
            if y_mag > x_mag {
                make(axis_y, axis_x, axis_z)
            } else if z_mag > y_mag {
                make(axis_x, axis_z, axis_y)
            } else {
                make(axis_x, axis_y, axis_z)
            }
        } else if y_mag > z_mag {
            make(axis_y, axis_z, axis_x)
        } else if x_mag > y_mag {
            make(axis_z, axis_x, axis_y)
        } else {
            make(axis_z, axis_y, axis_x)
        }
    }

    pub fn facing_axis(x_rot: f32, y_rot: f32, axis: Axis) -> Direction {
        match axis {
            Axis::X => {
                if East.is_facing_angle(y_rot) {
                    East
                } else {
                    West
                }
            }
            Axis::Y => {
                if x_rot < 0.0 {
                    Up
                } else {
                    Down
                }
            }
            Axis::Z => {
                if South.is_facing_angle(y_rot) {
                    South
                } else {
                    North
                }
            }
        }
    }

    pub fn is_facing_angle(self, y_angle: f32) -> bool {
        let radians = y_angle * mth::DEG_TO_RAD;
        let dx = -mth::sin(radians as f64);
        let dz = mth::cos(radians as f64);
        self.step_x() as f32 * dx + self.step_z() as f32 * dz > 0.0
    }

    pub fn approximate_nearest(dx: f32, dy: f32, dz: f32) -> Direction {
        let mut result = North;
        let mut highest = f32::from_bits(1);
        for d in Self::VALUES {
            let dot = dx * d.step_x() as f32 + dy * d.step_y() as f32 + dz * d.step_z() as f32;
            if dot > highest {
                highest = dot;
                result = d;
            }
        }
        result
    }

    pub fn nearest(x: i32, y: i32, z: i32) -> Option<Direction> {
        let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
        if ax > az && ax > ay {
            Some(if x < 0 { West } else { East })
        } else if az > ax && az > ay {
            Some(if z < 0 { North } else { South })
        } else if ay > ax && ay > az {
            Some(if y < 0 { Down } else { Up })
        } else {
            None
        }
    }
}

impl Axis {
    pub const VALUES: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    pub const fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }

    pub const fn is_vertical(self) -> bool {
        matches!(self, Axis::Y)
    }

    pub const fn is_horizontal(self) -> bool {
        !self.is_vertical()
    }

    pub const fn positive(self) -> Direction {
        match self {
            Axis::X => East,
            Axis::Y => Up,
            Axis::Z => South,
        }
    }

    pub const fn negative(self) -> Direction {
        match self {
            Axis::X => West,
            Axis::Y => Down,
            Axis::Z => North,
        }
    }

    pub fn choose<T>(self, x: T, y: T, z: T) -> T {
        match self {
            Axis::X => x,
            Axis::Y => y,
            Axis::Z => z,
        }
    }

    pub fn step_order(dx: f64, dz: f64) -> [Axis; 3] {
        if dx.abs() < dz.abs() { [Axis::Y, Axis::Z, Axis::X] } else { [Axis::Y, Axis::X, Axis::Z] }
    }
}

impl AxisDirection {
    pub const fn step(self) -> i32 {
        match self {
            AxisDirection::Positive => 1,
            AxisDirection::Negative => -1,
        }
    }

    pub const fn opposite(self) -> AxisDirection {
        match self {
            AxisDirection::Positive => AxisDirection::Negative,
            AxisDirection::Negative => AxisDirection::Positive,
        }
    }
}
