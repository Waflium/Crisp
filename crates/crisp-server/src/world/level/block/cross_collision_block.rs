use crisp_core::Direction;
use crisp_data::properties::{BooleanProperty, EAST, NORTH, SOUTH, WEST};

pub fn property_by_direction(direction: Direction) -> BooleanProperty {
    match direction {
        Direction::North => NORTH,
        Direction::East => EAST,
        Direction::South => SOUTH,
        Direction::West => WEST,
        _ => unreachable!("{direction:?} is not horizontal"),
    }
}
