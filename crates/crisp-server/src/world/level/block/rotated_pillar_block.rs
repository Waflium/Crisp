use crisp_core::Axis;
use crisp_data::properties::AXIS;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;

pub struct RotatedPillarBlock;

pub fn rotate_pillar(state: BlockState, rotation: Rotation) -> BlockState {
    match rotation {
        Rotation::Counterclockwise90 | Rotation::Clockwise90 => match state.get(AXIS) {
            Axis::X => state.set(AXIS, Axis::Z),
            Axis::Z => state.set(AXIS, Axis::X),
            Axis::Y => state,
        },
        _ => state,
    }
}

impl BlockBehaviour for RotatedPillarBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(AXIS, ctx.clicked_face().axis()))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        rotate_pillar(state, rotation)
    }
}
