use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use super::BlockBehaviour;
use super::bonemealable_block::{find_spreadable_neighbour_pos, has_spreadable_neighbour_pos};
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_ALL;

pub struct BushBlock;

impl Vegetation for BushBlock {}

impl BlockBehaviour for BushBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        has_spreadable_neighbour_pos(level, pos, state)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if let Some(neighbour_pos) = find_spreadable_neighbour_pos(level, pos, state) {
            level.set_block(neighbour_pos, state.block().default_state(), UPDATE_ALL);
        }
    }
}
