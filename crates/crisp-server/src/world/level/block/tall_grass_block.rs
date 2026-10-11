use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::double_plant_block;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct TallGrassBlock;

fn grown_block(state: BlockState) -> Block {
    if state.is_block(blocks::FERN) { blocks::LARGE_FERN } else { blocks::TALL_GRASS }
}

impl Vegetation for TallGrassBlock {}

impl BlockBehaviour for TallGrassBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        let grown = grown_block(state).default_state();
        super::of(grown).can_survive(grown, level, pos) && level.block_state(pos.above()).is_air() && !level.chunks.is_outside_build_height(pos.y + 1)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        double_plant_block::place_at(level, grown_block(state).default_state(), pos, UPDATE_CLIENTS);
    }
}
