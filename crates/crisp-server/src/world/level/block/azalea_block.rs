use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, block_tags};

use super::BlockBehaviour;
use super::grower::tree_grower::AZALEA;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub struct AzaleaBlock;

impl Vegetation for AzaleaBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_AZALEA)
    }
}

impl BlockBehaviour for AzaleaBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        let min_height = AZALEA.get_minimum_height().unwrap_or(0);
        level.is_inside_build_height(pos.relative_n(Direction::Up, min_height + 2).y) && level.fluid_state(pos.above()).is_empty()
    }

    fn is_bonemeal_success(&self, level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        (level.random.next_float() as f64) < 0.45
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        AZALEA.grow_tree(level, pos, state);
    }
}
