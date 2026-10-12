use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState, SupportType, blocks, fluid_tags};

use super::BlockBehaviour;
use super::simple_waterlogged_block::schedule_water_tick;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub fn try_schedule_die_tick(block: Block, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos) {
    if !scan_for_water(state, level, pos) {
        let delay = 60 + level.get_random().next_int_bounded(40);
        level.schedule_tick(pos, block, delay);
    }
}

pub fn scan_for_water(state: BlockState, level: &mut dyn LevelReader, block_pos: BlockPos) -> bool {
    if state.get(WATERLOGGED) {
        return true;
    }
    Direction::VALUES.iter().any(|&direction| level.fluid_state(block_pos.relative(direction)).is(&fluid_tags::WATER))
}

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> BlockState {
    let fluid_state = level.fluid_state(ctx.clicked_pos());
    block.default_state().set(WATERLOGGED, fluid_state.is(&fluid_tags::WATER) && fluid_state.amount == 8)
}

pub fn update_shape(behaviour: &dyn BlockBehaviour, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction) -> BlockState {
    schedule_water_tick(state, level, pos);
    if direction == Direction::Down && !behaviour.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
}

pub fn can_survive(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    let below = pos.below();
    level.block_state(below).is_face_sturdy(Direction::Up, SupportType::Full)
}

pub struct BaseCoralPlantTypeBlock;

impl BlockBehaviour for BaseCoralPlantTypeBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(get_state_for_placement(block, ctx, level))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        update_shape(self, state, level, pos, direction)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        can_survive(level, pos)
    }
}
