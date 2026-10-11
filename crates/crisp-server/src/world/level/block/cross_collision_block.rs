use crisp_core::Direction;
use crisp_data::BlockState;
use crisp_data::properties::{BooleanProperty, EAST, NORTH, SOUTH, WEST};

use super::mirror::Mirror;
use super::rotation::Rotation;

pub fn property_by_direction(direction: Direction) -> BooleanProperty {
    match direction {
        Direction::North => NORTH,
        Direction::East => EAST,
        Direction::South => SOUTH,
        Direction::West => WEST,
        _ => unreachable!("{direction:?} is not horizontal"),
    }
}

pub fn rotate(state: BlockState, rotation: Rotation) -> BlockState {
    match rotation {
        Rotation::Clockwise180 => state.set(NORTH, state.get(SOUTH)).set(EAST, state.get(WEST)).set(SOUTH, state.get(NORTH)).set(WEST, state.get(EAST)),
        Rotation::Counterclockwise90 => state.set(NORTH, state.get(EAST)).set(EAST, state.get(SOUTH)).set(SOUTH, state.get(WEST)).set(WEST, state.get(NORTH)),
        Rotation::Clockwise90 => state.set(NORTH, state.get(WEST)).set(EAST, state.get(NORTH)).set(SOUTH, state.get(EAST)).set(WEST, state.get(SOUTH)),
        Rotation::None => state,
    }
}

pub fn mirror(state: BlockState, mirror: Mirror) -> BlockState {
    match mirror {
        Mirror::LeftRight => state.set(NORTH, state.get(SOUTH)).set(SOUTH, state.get(NORTH)),
        Mirror::FrontBack => state.set(EAST, state.get(WEST)).set(WEST, state.get(EAST)),
        Mirror::None => state,
    }
}
