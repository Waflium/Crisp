use crisp_core::{BlockPos, Direction};
use crisp_data::properties::LIT;
use crisp_data::{Block, BlockState};

use super::{BlockBehaviour, base_torch_block};
use crate::server::level::ServerLevel;
use crate::server::level::events::REDSTONE_TORCH_BURNOUT;
use crate::world::level::{BlockGetter, LevelReader};

const RECENT_TOGGLE_TIMER: i64 = 60;
const MAX_RECENT_TOGGLES: i32 = 8;
const RESTART_DELAY: i32 = 160;
const TOGGLE_DELAY: i32 = 2;

pub struct RedstoneTorchBlock {
    pub block: Block,
}

impl RedstoneTorchBlock {
    fn notify_neighbors(&self, level: &mut ServerLevel, pos: BlockPos) {
        for direction in Direction::VALUES {
            level.update_neighbors_at(pos.relative(direction), self.block);
        }
    }

    pub fn tick_with_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, neighbor_signal: bool) {
        let toggles = &mut level.redstone_torch_toggles;
        while !toggles.is_empty() && level.game_time - toggles[0].1 > RECENT_TOGGLE_TIMER {
            toggles.remove(0);
        }
        if state.get(LIT) {
            if neighbor_signal {
                level.set_block_simple(pos, state.set(LIT, false));
                if is_toggled_too_frequently(level, pos, true) {
                    level.level_event(None, REDSTONE_TORCH_BURNOUT, pos, 0);
                    let block = level.block_state(pos).block();
                    level.schedule_tick(pos, block, RESTART_DELAY);
                }
            }
        } else if !neighbor_signal && !is_toggled_too_frequently(level, pos, false) {
            level.set_block_simple(pos, state.set(LIT, true));
        }
    }

    pub fn neighbor_changed_with_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, neighbor_signal: bool) {
        if state.get(LIT) == neighbor_signal && !level.will_tick_this_tick(pos, self.block) {
            level.schedule_tick(pos, self.block, TOGGLE_DELAY);
        }
    }

    pub fn on_place(&self, level: &mut ServerLevel, pos: BlockPos) {
        self.notify_neighbors(level, pos);
    }

    pub fn affect_neighbors_after_removal(&self, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        if !moved_by_piston {
            self.notify_neighbors(level, pos);
        }
    }
}

fn is_toggled_too_frequently(level: &mut ServerLevel, pos: BlockPos, add: bool) -> bool {
    if add {
        let when = level.game_time;
        level.redstone_torch_toggles.push((pos, when));
    }
    let mut count = 0;
    for (toggle_pos, _) in &level.redstone_torch_toggles {
        if *toggle_pos == pos {
            count += 1;
            if count >= MAX_RECENT_TOGGLES {
                return true;
            }
        }
    }
    false
}

pub fn own_signal(state: BlockState) -> i32 {
    if state.get(LIT) { 15 } else { 0 }
}

pub fn get_direct_signal(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
    if direction == Direction::Down { super::of(state).get_signal(state, level, pos, direction) } else { 0 }
}

impl BlockBehaviour for RedstoneTorchBlock {
    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        base_torch_block::update_shape(state, level, pos, direction)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        base_torch_block::can_survive(level, pos)
    }

    fn on_place(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        RedstoneTorchBlock::on_place(self, level, pos);
    }

    fn affect_neighbors_after_removal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        RedstoneTorchBlock::affect_neighbors_after_removal(self, level, pos, moved_by_piston);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let neighbor_signal = level.has_signal(pos.below(), Direction::Down);
        self.tick_with_signal(state, level, pos, neighbor_signal);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let neighbor_signal = level.has_signal(pos.below(), Direction::Down);
        self.neighbor_changed_with_signal(state, level, pos, neighbor_signal);
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        get_direct_signal(state, level, pos, direction)
    }

    fn own_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        own_signal(state)
    }

    fn get_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, direction: Direction) -> i32 {
        if direction != Direction::Up { own_signal(state) } else { 0 }
    }
}
