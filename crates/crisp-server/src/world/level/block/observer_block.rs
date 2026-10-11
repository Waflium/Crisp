use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{FACING, POWERED};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_KNOWN_SHAPE};

pub struct ObserverBlock {
    pub block: Block,
}

impl ObserverBlock {
    fn start_signal(&self, level: &mut dyn LevelAccessor, pos: BlockPos) {
        if !level.has_scheduled_tick(pos, self.block) {
            level.schedule_tick(pos, self.block, 2);
        }
    }

    fn update_neighbors_in_front(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let direction = state.get(FACING);
        let opposite_pos = pos.relative(direction.opposite());
        level.neighbor_changed(opposite_pos, self.block);
        level.update_neighbors_at_except_from_facing(opposite_pos, self.block, Some(direction));
    }
}

impl BlockBehaviour for ObserverBlock {
    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(POWERED) {
            level.set_block(pos, state.set(POWERED, false), UPDATE_CLIENTS);
        } else {
            level.set_block(pos, state.set(POWERED, true), UPDATE_CLIENTS);
            level.schedule_tick(pos, self.block, 2);
        }
        self.update_neighbors_in_front(level, pos, state);
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if state.get(FACING) == direction && !state.get(POWERED) {
            self.start_signal(level, pos);
        }
        state
    }

    fn should_redstone_wire_connect_to(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos, direction: Option<Direction>) -> bool {
        direction == Some(state.get(FACING))
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        super::of(state).get_signal(state, level, pos, direction)
    }

    fn get_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        if state.get(FACING) == direction { self.own_signal(state, level, pos) } else { 0 }
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !state.is_block(old_state.block()) && state.get(POWERED) && !level.has_scheduled_tick(pos, self.block) {
            let new_state = state.set(POWERED, false);
            level.set_block(pos, new_state, UPDATE_CLIENTS | UPDATE_KNOWN_SHAPE);
            self.update_neighbors_in_front(level, pos, new_state);
        }
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        if state.get(POWERED) && level.has_scheduled_tick(pos, self.block) {
            self.update_neighbors_in_front(level, pos, state.set(POWERED, false));
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(FACING, ctx.nearest_looking_direction()))
    }
}
