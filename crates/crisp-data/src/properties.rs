use std::marker::PhantomData;

pub use crisp_core::{Axis, Direction};

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

pub type BooleanProperty = Property<bool>;
pub type IntegerProperty = Property<i32>;
pub type EnumProperty<T> = Property<T>;

include!(concat!(env!("OUT_DIR"), "/properties.rs"));
