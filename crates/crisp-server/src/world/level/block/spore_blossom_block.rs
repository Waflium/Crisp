use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};

use super::{BlockBehaviour, can_support_center};
use crate::server::level::ServerLevel;

pub struct SporeBlossomBlock;

impl BlockBehaviour for SporeBlossomBlock {
    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        can_support_center(level, pos.above(), Direction::Down) && !level.is_water_at(pos)
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Up && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }
}
