use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf};
use crisp_data::{Block, BlockState, SupportType, block_tags, blocks, fluid_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub fn may_place_on_seagrass_support(state: BlockState) -> bool {
    state.is_face_sturdy(Direction::Up, SupportType::Full) && !state.is(&block_tags::CANNOT_SUPPORT_SEAGRASS)
}

pub struct SeagrassBlock;

impl Vegetation for SeagrassBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        may_place_on_seagrass_support(state)
    }
}

impl BlockBehaviour for SeagrassBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let fluid = level.fluid_state(ctx.clicked_pos());
        (fluid.is(&fluid_tags::WATER) && fluid.amount == 8).then(|| block.default_state())
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        let result = vegetation_block::update_shape(state, level, pos);
        if !result.is_air() {
            schedule_water_tick(level, pos);
        }
        result
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        level.block_state(pos.above()).is_block(blocks::WATER)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        let lower_state = blocks::TALL_SEAGRASS.default_state();
        let upper_state = lower_state.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper);
        level.set_block(pos, lower_state, UPDATE_CLIENTS);
        level.set_block(pos.above(), upper_state, UPDATE_CLIENTS);
    }
}
