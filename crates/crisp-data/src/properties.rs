use std::marker::PhantomData;

pub use crisp_core::{Axis, Direction};
use crisp_core::math::OctahedralGroup;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PropertyId(pub u16);

pub struct Property<T> {
    pub id: PropertyId,
    _value: PhantomData<fn() -> T>,
}

impl<T> Property<T> {
    pub const fn new(id: u16) -> Property<T> {
        Property { id: PropertyId(id), _value: PhantomData }
    }

    pub fn name(self) -> &'static str {
        crate::PROPERTIES[self.id.0 as usize].name
    }
}

impl<T> Clone for Property<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Property<T> {}

impl<T> PartialEq for Property<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub trait PropertyValue: Copy {
    fn from_ordinal(ordinal: u8) -> Self;
    fn ordinal(self) -> u8;
}

pub(crate) struct PropertyData {
    pub name: &'static str,
    pub values: &'static [&'static str],
    pub ordinals: &'static [u8],
}

impl PropertyValue for bool {
    fn from_ordinal(ordinal: u8) -> bool {
        ordinal != 0
    }
    fn ordinal(self) -> u8 {
        self as u8
    }
}

impl PropertyValue for i32 {
    fn from_ordinal(ordinal: u8) -> i32 {
        ordinal as i32
    }
    fn ordinal(self) -> u8 {
        self as u8
    }
}

impl PropertyValue for Direction {
    fn from_ordinal(ordinal: u8) -> Direction {
        Direction::VALUES[ordinal as usize]
    }
    fn ordinal(self) -> u8 {
        self as u8
    }
}

impl PropertyValue for Axis {
    fn from_ordinal(ordinal: u8) -> Axis {
        Axis::VALUES[ordinal as usize]
    }
    fn ordinal(self) -> u8 {
        self as u8
    }
}

impl FrontAndTop {
    pub const fn from_front_and_top(front: Direction, top: Direction) -> FrontAndTop {
        match (front, top) {
            (Direction::Down, Direction::East) => FrontAndTop::DownEast,
            (Direction::Down, Direction::North) => FrontAndTop::DownNorth,
            (Direction::Down, Direction::South) => FrontAndTop::DownSouth,
            (Direction::Down, Direction::West) => FrontAndTop::DownWest,
            (Direction::Up, Direction::East) => FrontAndTop::UpEast,
            (Direction::Up, Direction::North) => FrontAndTop::UpNorth,
            (Direction::Up, Direction::South) => FrontAndTop::UpSouth,
            (Direction::Up, Direction::West) => FrontAndTop::UpWest,
            (Direction::West, Direction::Up) => FrontAndTop::WestUp,
            (Direction::East, Direction::Up) => FrontAndTop::EastUp,
            (Direction::North, Direction::Up) => FrontAndTop::NorthUp,
            (Direction::South, Direction::Up) => FrontAndTop::SouthUp,
            _ => panic!("no FrontAndTop for this front and top"),
        }
    }

    pub const fn front(self) -> Direction {
        match self {
            FrontAndTop::DownEast | FrontAndTop::DownNorth | FrontAndTop::DownSouth | FrontAndTop::DownWest => Direction::Down,
            FrontAndTop::UpEast | FrontAndTop::UpNorth | FrontAndTop::UpSouth | FrontAndTop::UpWest => Direction::Up,
            FrontAndTop::WestUp => Direction::West,
            FrontAndTop::EastUp => Direction::East,
            FrontAndTop::NorthUp => Direction::North,
            FrontAndTop::SouthUp => Direction::South,
        }
    }

    pub const fn top(self) -> Direction {
        match self {
            FrontAndTop::DownEast | FrontAndTop::UpEast => Direction::East,
            FrontAndTop::DownNorth | FrontAndTop::UpNorth => Direction::North,
            FrontAndTop::DownSouth | FrontAndTop::UpSouth => Direction::South,
            FrontAndTop::DownWest | FrontAndTop::UpWest => Direction::West,
            FrontAndTop::WestUp | FrontAndTop::EastUp | FrontAndTop::NorthUp | FrontAndTop::SouthUp => Direction::Up,
        }
    }

    pub const fn rotate(self, group: OctahedralGroup) -> FrontAndTop {
        FrontAndTop::from_front_and_top(group.rotate(self.front()), group.rotate(self.top()))
    }
}

impl SideChainPart {
    pub fn is_connected(self) -> bool {
        self != SideChainPart::Unconnected
    }

    pub fn is_connection_towards(self, end_part: SideChainPart) -> bool {
        self == SideChainPart::Center || self == end_part
    }

    pub fn is_chain_end(self) -> bool {
        self != SideChainPart::Center
    }

    pub fn when_connected_to_the_right(self) -> SideChainPart {
        match self {
            SideChainPart::Unconnected | SideChainPart::Left => SideChainPart::Left,
            SideChainPart::Right | SideChainPart::Center => SideChainPart::Center,
        }
    }

    pub fn when_connected_to_the_left(self) -> SideChainPart {
        match self {
            SideChainPart::Unconnected | SideChainPart::Right => SideChainPart::Right,
            SideChainPart::Center | SideChainPart::Left => SideChainPart::Center,
        }
    }

    pub fn when_disconnected_from_the_right(self) -> SideChainPart {
        match self {
            SideChainPart::Unconnected | SideChainPart::Left => SideChainPart::Unconnected,
            SideChainPart::Right | SideChainPart::Center => SideChainPart::Right,
        }
    }

    pub fn when_disconnected_from_the_left(self) -> SideChainPart {
        match self {
            SideChainPart::Unconnected | SideChainPart::Right => SideChainPart::Unconnected,
            SideChainPart::Center | SideChainPart::Left => SideChainPart::Left,
        }
    }
}

pub type BooleanProperty = Property<bool>;
pub type IntegerProperty = Property<i32>;
pub type EnumProperty<T> = Property<T>;

include!(concat!(env!("OUT_DIR"), "/properties.rs"));
