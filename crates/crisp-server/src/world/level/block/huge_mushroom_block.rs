use crisp_core::Direction;
use crisp_data::BlockState;
use crisp_data::properties::{DOWN, EAST, NORTH, SOUTH, UP, WEST};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::multiface_block::get_face_property as property_by_direction;
use super::rotation::Rotation;

pub struct HugeMushroomBlock;

impl BlockBehaviour for HugeMushroomBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state
            .set(property_by_direction(rotation.rotate(Direction::North)), state.get(NORTH))
            .set(property_by_direction(rotation.rotate(Direction::South)), state.get(SOUTH))
            .set(property_by_direction(rotation.rotate(Direction::East)), state.get(EAST))
            .set(property_by_direction(rotation.rotate(Direction::West)), state.get(WEST))
            .set(property_by_direction(rotation.rotate(Direction::Up)), state.get(UP))
            .set(property_by_direction(rotation.rotate(Direction::Down)), state.get(DOWN))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state
            .set(property_by_direction(mirror.mirror(Direction::North)), state.get(NORTH))
            .set(property_by_direction(mirror.mirror(Direction::South)), state.get(SOUTH))
            .set(property_by_direction(mirror.mirror(Direction::East)), state.get(EAST))
            .set(property_by_direction(mirror.mirror(Direction::West)), state.get(WEST))
            .set(property_by_direction(mirror.mirror(Direction::Up)), state.get(UP))
            .set(property_by_direction(mirror.mirror(Direction::Down)), state.get(DOWN))
    }
}
