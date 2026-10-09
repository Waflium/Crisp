use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, POWER, POWERED};
use crisp_data::{Block, BlockState, SupportType, blocks};

use super::is_diode;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::ticks::TickPriority;

#[allow(unused_variables)]
pub trait DiodeBlock {
    fn block(&self) -> Block;

    fn get_delay(&self, state: BlockState) -> i32;

    fn is_locked(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        false
    }

    fn side_input_diodes_only(&self) -> bool {
        false
    }

    fn get_output_signal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> i32 {
        15
    }

    fn should_turn_on(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        self.get_input_signal(level, pos, state) > 0
    }

    fn get_input_signal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> i32 {
        let direction = state.get(HORIZONTAL_FACING);
        let target_pos = pos.relative(direction);
        let input = level.get_signal(target_pos, direction);
        if input >= 15 {
            return input;
        }
        let target_state = level.block_state(target_pos);
        input.max(if target_state.is_block(blocks::REDSTONE_WIRE) { target_state.get(POWER) } else { 0 })
    }

    fn get_alternate_signal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> i32 {
        let direction = state.get(HORIZONTAL_FACING);
        let clockwise = direction.clockwise();
        let counter_clockwise = direction.counter_clockwise();
        let side_input_diodes_only = self.side_input_diodes_only();
        let right = level.get_control_input_signal(pos.relative(clockwise), clockwise, side_input_diodes_only);
        let left = level.get_control_input_signal(pos.relative(counter_clockwise), counter_clockwise, side_input_diodes_only);
        right.max(left)
    }

    fn check_tick_on_neighbor(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if !self.is_locked(level, pos, state) {
            let on = state.get(POWERED);
            let should_turn_on = self.should_turn_on(level, pos, state);
            if on != should_turn_on && !level.will_tick_this_tick(pos, self.block()) {
                let priority = if should_prioritize(level, pos, state) {
                    TickPriority::EXTREMELY_HIGH
                } else if on {
                    TickPriority::VERY_HIGH
                } else {
                    TickPriority::HIGH
                };
                level.schedule_tick_with_priority(pos, self.block(), self.get_delay(state), priority);
            }
        }
    }

    fn update_neighbors_in_front(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let direction = state.get(HORIZONTAL_FACING);
        let opposite_pos = pos.relative(direction.opposite());
        level.neighbor_changed(opposite_pos, self.block());
        level.update_neighbors_at_except_from_facing(opposite_pos, self.block(), Some(direction));
    }
}

pub fn should_prioritize(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
    let direction = state.get(HORIZONTAL_FACING).opposite();
    let opposite_state = level.block_state(pos.relative(direction));
    is_diode(opposite_state) && opposite_state.get(HORIZONTAL_FACING) != direction
}

pub fn can_survive_on(neighbor_state: BlockState) -> bool {
    neighbor_state.is_face_sturdy(Direction::Up, SupportType::Rigid)
}

pub fn can_survive(level: &mut ServerLevel, pos: BlockPos) -> bool {
    can_survive_on(level.block_state(pos.below()))
}

pub fn tick(diode: &(impl DiodeBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if !diode.is_locked(level, pos, state) {
        let on = state.get(POWERED);
        let should_turn_on = diode.should_turn_on(level, pos, state);
        if on && !should_turn_on {
            level.set_block(pos, state.set(POWERED, false), UPDATE_CLIENTS);
        } else if !on {
            level.set_block(pos, state.set(POWERED, true), UPDATE_CLIENTS);
            if !should_turn_on {
                level.schedule_tick_with_priority(pos, diode.block(), diode.get_delay(state), TickPriority::VERY_HIGH);
            }
        }
    }
}

pub fn neighbor_changed(diode: &(impl DiodeBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if level.block_state(pos).is_block(diode.block()) {
        if super::of(state).can_survive(state, level, pos) {
            diode.check_tick_on_neighbor(level, pos, state);
        } else {
            level.drop_resources(state, pos, None, None);
            level.remove_block(pos, false);
            for direction in Direction::VALUES {
                level.update_neighbors_at(pos.relative(direction), diode.block());
            }
        }
    }
}

pub fn own_signal(diode: &(impl DiodeBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> i32 {
    if state.get(POWERED) { diode.get_output_signal(level, pos, state) } else { 0 }
}

pub fn get_signal(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
    if state.get(HORIZONTAL_FACING) == direction { super::of(state).own_signal(state, level, pos) } else { 0 }
}

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext) -> BlockState {
    block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite())
}

pub fn set_placed_by(diode: &(impl DiodeBlock + ?Sized), level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    if diode.should_turn_on(level, pos, state) {
        level.schedule_tick(pos, diode.block(), 1);
    }
}

pub fn affect_neighbors_after_removal(diode: &(impl DiodeBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
    if !moved_by_piston {
        diode.update_neighbors_in_front(level, pos, state);
    }
}
