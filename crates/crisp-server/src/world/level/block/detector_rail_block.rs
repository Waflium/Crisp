use crisp_core::{BlockPos, Direction};
use crisp_data::properties::POWERED;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::base_rail_block::{self as base, BaseRailBlock};
use super::rail_state::RailState;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

const PRESSED_CHECK_PERIOD: i32 = 20;

pub struct DetectorRailBlock {
    pub block: Block,
}

impl DetectorRailBlock {
    fn check_pressed(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if base::can_survive(level, pos) && level.block_state(pos).is_block(self.block) {
            let was_pressed = state.get(POWERED);
            let should_be_pressed = false;
            if should_be_pressed && !was_pressed {
                let new_state = state.set(POWERED, true);
                level.set_block_simple(pos, new_state);
                self.update_power_to_connected(level, pos, new_state);
                level.update_neighbors_at(pos, self.block);
                level.update_neighbors_at(pos.below(), self.block);
            }
            if !should_be_pressed && was_pressed {
                let new_state = state.set(POWERED, false);
                level.set_block_simple(pos, new_state);
                self.update_power_to_connected(level, pos, new_state);
                level.update_neighbors_at(pos, self.block);
                level.update_neighbors_at(pos.below(), self.block);
            }
            if should_be_pressed {
                level.schedule_tick(pos, self.block, PRESSED_CHECK_PERIOD);
            }
            level.update_neighbour_for_output_signal(pos, self.block);
        }
    }

    fn update_power_to_connected(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let rail = RailState::new(pos, state);
        for &connection_pos in rail.connections() {
            let connection_state = level.block_state(connection_pos);
            level.neighbor_changed_with_state(connection_state, connection_pos, connection_state.block(), false);
        }
    }
}

impl BaseRailBlock for DetectorRailBlock {
    fn block(&self) -> Block {
        self.block
    }
}

impl BlockBehaviour for DetectorRailBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(base::get_state_for_placement(block, ctx, level))
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        base::can_survive(level, pos)
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {
        if !old_state.is_block(state.block()) {
            let updated_state = base::update_state(self, state, level, pos, moved_by_piston);
            self.check_pressed(level, pos, updated_state);
        }
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        base::neighbor_changed(self, state, level, pos, block, moved_by_piston);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        base::affect_neighbors_after_removal(self, state, level, pos, moved_by_piston);
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        base::update_shape(state, level, pos)
    }

    fn own_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }

    fn get_direct_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, direction: Direction) -> i32 {
        if !state.get(POWERED) {
            0
        } else if direction == Direction::Up {
            15
        } else {
            0
        }
    }

    fn entity_inside(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _entity: EntityId,
        _effects: &mut InsideBlockEffects,
        _is_precise: bool,
    ) {
        if !state.get(POWERED) {
            self.check_pressed(level, pos, state);
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(POWERED) {
            self.check_pressed(level, pos, state);
        }
    }
}
