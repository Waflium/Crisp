use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::base_coral_plant_type_block::{self, BaseCoralPlantTypeBlock, scan_for_water, try_schedule_die_tick};
use super::simple_waterlogged_block::schedule_water_tick;
use super::vegetation_block::{block_field, fields};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct CoralFanBlock {
    block: Block,
    dead_block: Block,
}

impl CoralFanBlock {
    pub fn new(block: Block) -> CoralFanBlock {
        CoralFanBlock { block, dead_block: block_field(&fields(block), "deadBlock") }
    }
}

impl BlockBehaviour for CoralFanBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(base_coral_plant_type_block::get_state_for_placement(block, ctx, level))
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        try_schedule_die_tick(self.block, state, level, pos);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !scan_for_water(state, level, pos) {
            level.set_block(pos, self.dead_block.default_state().set(WATERLOGGED, false), UPDATE_CLIENTS);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        if direction == Direction::Down && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        try_schedule_die_tick(self.block, state, level, pos);
        schedule_water_tick(state, level, pos);
        BaseCoralPlantTypeBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        base_coral_plant_type_block::can_survive(level, pos)
    }
}
