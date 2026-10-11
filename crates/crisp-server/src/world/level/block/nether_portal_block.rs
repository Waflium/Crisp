use crisp_core::Axis;
use crisp_data::BlockState;
use crisp_data::properties::HORIZONTAL_AXIS;

use super::BlockBehaviour;
use super::rotation::Rotation;

pub struct NetherPortalBlock;

impl BlockBehaviour for NetherPortalBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        match rotation {
            Rotation::Counterclockwise90 | Rotation::Clockwise90 => match state.get(HORIZONTAL_AXIS) {
                Axis::X => state.set(HORIZONTAL_AXIS, Axis::Z),
                Axis::Z => state.set(HORIZONTAL_AXIS, Axis::X),
                Axis::Y => state,
            },
            _ => state,
        }
    }
}
