use crisp_core::Direction;
use crisp_data::properties::{BooleanProperty, DOWN, EAST, NORTH, SOUTH, UP, WEST};

pub fn property_by_direction(direction: Direction) -> BooleanProperty {
    match direction {
        Direction::North => NORTH,
        Direction::East => EAST,
        Direction::South => SOUTH,
        Direction::West => WEST,
        Direction::Up => UP,
        Direction::Down => DOWN,
    }
}
