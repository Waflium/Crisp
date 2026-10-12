use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::base_coral_plant_type_block::{scan_for_water, try_schedule_die_tick};
use super::base_coral_wall_fan_block::BaseCoralWallFanBlock;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::schedule_water_tick;
use super::vegetation_block::{block_field, fields};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct CoralWallFanBlock {
    block: Block,
    dead_block: Block,
}

impl CoralWallFanBlock {
    pub fn new(block: Block) -> CoralWallFanBlock {
        CoralWallFanBlock { block, dead_block: block_field(&fields(block), "deadBlock") }
    }
}

impl BlockBehaviour for CoralWallFanBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        BaseCoralWallFanBlock.get_state_for_placement(block, ctx, level)
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        try_schedule_die_tick(self.block, state, level, pos);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !scan_for_water(state, level, pos) {
            let dead = self.dead_block.default_state().set(WATERLOGGED, false).set(HORIZONTAL_FACING, state.get(HORIZONTAL_FACING));
            level.set_block(pos, dead, UPDATE_CLIENTS);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        if direction.opposite() == state.get(HORIZONTAL_FACING) && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        schedule_water_tick(state, level, pos);
        try_schedule_die_tick(self.block, state, level, pos);
        BaseCoralWallFanBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        BaseCoralWallFanBlock.can_survive(state, level, pos)
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        BaseCoralWallFanBlock.rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        BaseCoralWallFanBlock.mirror(state, mirror)
    }
}
