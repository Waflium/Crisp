use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use super::bonemealable_block::{find_spreadable_neighbour_pos, has_spreadable_neighbour_pos};
use super::dry_vegetation_block::DryVegetationBlock;
use super::vegetation_block;
use crate::server::level::ServerLevel;
use crate::world::level::LevelReader;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct TallDryGrassBlock;

impl BlockBehaviour for TallDryGrassBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(&DryVegetationBlock, level, pos)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        has_spreadable_neighbour_pos(level, pos, blocks::SHORT_DRY_GRASS.default_state())
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        if let Some(neighbour_pos) = find_spreadable_neighbour_pos(level, pos, blocks::SHORT_DRY_GRASS.default_state()) {
            level.set_block(neighbour_pos, blocks::SHORT_DRY_GRASS.default_state(), UPDATE_ALL);
        }
    }
}
