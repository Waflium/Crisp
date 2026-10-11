use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use super::{BlockBehaviour, base_torch_block};
use crate::server::level::ServerLevel;
use crate::world::level::LevelReader;

pub struct TorchBlock;

impl BlockBehaviour for TorchBlock {
    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        base_torch_block::update_shape(state, level, pos, direction)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        base_torch_block::can_survive(level, pos)
    }
}
