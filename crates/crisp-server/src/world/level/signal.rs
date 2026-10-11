use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;
use crisp_data::properties::POWER;

use super::{BlockGetter, LevelReader};
use super::block;
use crate::server::level::ServerLevel;

pub fn get_direct_signal(level: &mut impl LevelReader, pos: BlockPos, direction: Direction) -> i32 {
    let state = level.block_state(pos);
    block::of(state).get_direct_signal(state, level, pos, direction)
}

pub fn get_direct_signal_to(level: &mut impl LevelReader, pos: BlockPos) -> i32 {
    let mut result = 0;
    for direction in [Direction::Down, Direction::Up, Direction::North, Direction::South, Direction::West, Direction::East] {
        result = result.max(get_direct_signal(level, pos.relative(direction), direction));
        if result >= 15 {
            return result;
        }
    }
    result
}

pub fn get_control_input_signal(level: &mut impl LevelReader, pos: BlockPos, direction: Direction, only_diodes: bool) -> i32 {
    let state = level.block_state(pos);
    if only_diodes {
        if block::is_diode(state) { get_direct_signal(level, pos, direction) } else { 0 }
    } else if state.is_block(blocks::REDSTONE_BLOCK) {
        15
    } else if state.is_block(blocks::REDSTONE_WIRE) {
        state.get(POWER)
    } else if state.is_signal_source() {
        get_direct_signal(level, pos, direction)
    } else {
        0
    }
}

pub fn has_signal(level: &mut impl LevelReader, pos: BlockPos, direction: Direction) -> bool {
    get_signal(level, pos, direction) > 0
}

pub fn get_signal(level: &mut impl LevelReader, pos: BlockPos, direction: Direction) -> i32 {
    let state = level.block_state(pos);
    let signal = block::of(state).get_signal(state, level, pos, direction);
    if state.is_redstone_conductor() { signal.max(get_direct_signal_to(level, pos)) } else { signal }
}

pub fn get_best_own_or_neighbour_signal(level: &mut impl LevelReader, pos: BlockPos) -> i32 {
    let state = level.block_state(pos);
    let own = if state.is_signal_source() { block::of(state).own_signal(state, level, pos) } else { 0 };
    get_best_neighbor_signal(level, pos).max(own)
}

pub fn has_neighbor_signal(level: &mut impl LevelReader, pos: BlockPos) -> bool {
    [Direction::Down, Direction::Up, Direction::North, Direction::South, Direction::West, Direction::East]
        .into_iter()
        .any(|d| get_signal(level, pos.relative(d), d) > 0)
}

pub fn get_best_neighbor_signal(level: &mut impl LevelReader, pos: BlockPos) -> i32 {
    let mut best = 0;
    for direction in Direction::VALUES {
        let signal = get_signal(level, pos.relative(direction), direction);
        if signal >= 15 {
            return 15;
        }
        best = best.max(signal);
    }
    best
}

impl ServerLevel {
    pub fn get_direct_signal(&mut self, pos: BlockPos, direction: Direction) -> i32 {
        get_direct_signal(self, pos, direction)
    }

    pub fn get_direct_signal_to(&mut self, pos: BlockPos) -> i32 {
        get_direct_signal_to(self, pos)
    }

    pub fn get_control_input_signal(&mut self, pos: BlockPos, direction: Direction, only_diodes: bool) -> i32 {
        get_control_input_signal(self, pos, direction, only_diodes)
    }

    pub fn has_signal(&mut self, pos: BlockPos, direction: Direction) -> bool {
        has_signal(self, pos, direction)
    }

    pub fn get_signal(&mut self, pos: BlockPos, direction: Direction) -> i32 {
        get_signal(self, pos, direction)
    }

    pub fn get_best_own_or_neighbour_signal(&mut self, pos: BlockPos) -> i32 {
        get_best_own_or_neighbour_signal(self, pos)
    }

    pub fn has_neighbor_signal(&mut self, pos: BlockPos) -> bool {
        has_neighbor_signal(self, pos)
    }

    pub fn get_best_neighbor_signal(&mut self, pos: BlockPos) -> i32 {
        get_best_neighbor_signal(self, pos)
    }

    pub fn update_neighbour_for_output_signal(&mut self, pos: BlockPos, changed: crisp_data::Block) {
        for direction in Direction::PLANE_HORIZONTAL {
            let mut relative = pos.relative(direction);
            if !self.chunks.has_chunk(relative.chunk()) {
                continue;
            }
            let mut state = self.block_state(relative);
            if state.is_block(blocks::COMPARATOR) {
                self.neighbor_changed_with_state(state, relative, changed, false);
            } else if state.is_redstone_conductor() {
                relative = relative.relative(direction);
                state = self.block_state(relative);
                if state.is_block(blocks::COMPARATOR) {
                    self.neighbor_changed_with_state(state, relative, changed, false);
                }
            }
        }
    }
}
