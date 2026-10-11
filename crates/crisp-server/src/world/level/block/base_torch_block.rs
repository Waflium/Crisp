use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};

use super::can_support_center;
use crate::server::level::ServerLevel;
use crate::world::level::LevelReader;

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> BlockState {
    if direction == Direction::Down && !super::of(state).can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
}

pub fn can_survive(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    can_support_center(level, pos.below(), Direction::Up)
}
