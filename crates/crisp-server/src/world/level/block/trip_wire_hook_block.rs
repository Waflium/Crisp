use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{ATTACHED, DISARMED, HORIZONTAL_FACING, POWERED};
use crisp_data::{Block, BlockState, blocks, registry, registry_id};

use super::BlockBehaviour;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

const WIRE_DIST_MAX: i32 = 42;
const RECHECK_PERIOD: i32 = 10;

pub struct TripWireHookBlock;

pub fn calculate_state(
    level: &mut ServerLevel,
    pos: BlockPos,
    state: BlockState,
    is_being_destroyed: bool,
    can_update: bool,
    wire_source: i32,
    wire_source_state: Option<BlockState>,
) {
    let Some(direction) = state.try_get(HORIZONTAL_FACING) else { return };
    let was_attached = state.try_get(ATTACHED).unwrap_or(false);
    let was_powered = state.try_get(POWERED).unwrap_or(false);
    let block = state.block();
    let mut attached = !is_being_destroyed;
    let mut powered = false;
    let mut receiver_pos = 0;
    let mut wire_states: [Option<BlockState>; WIRE_DIST_MAX as usize] = [None; WIRE_DIST_MAX as usize];

    for i in 1..WIRE_DIST_MAX {
        let test_pos = pos.relative_n(direction, i);
        let mut wire_state = level.block_state(test_pos);
        if wire_state.is_block(blocks::TRIPWIRE_HOOK) {
            if wire_state.get(HORIZONTAL_FACING) == direction.opposite() {
                receiver_pos = i;
            }
            break;
        }
        if !wire_state.is_block(blocks::TRIPWIRE) && i != wire_source {
            wire_states[i as usize] = None;
            attached = false;
        } else {
            if i == wire_source {
                wire_state = wire_source_state.unwrap_or(wire_state);
            }
            let wire_armed = !wire_state.get(DISARMED);
            let wire_powered = wire_state.get(POWERED);
            powered |= wire_armed && wire_powered;
            wire_states[i as usize] = Some(wire_state);
            if i == wire_source {
                level.schedule_tick(pos, block, RECHECK_PERIOD);
                attached &= wire_armed;
            }
        }
    }

    attached &= receiver_pos > 1;
    powered &= attached;
    let new_state = block.default_state().try_set(ATTACHED, attached).try_set(POWERED, powered);
    if receiver_pos > 0 {
        let test_pos = pos.relative_n(direction, receiver_pos);
        let opposite = direction.opposite();
        level.set_block_simple(test_pos, new_state.set(HORIZONTAL_FACING, opposite));
        notify_neighbors(block, level, test_pos, opposite);
        if !level.block_state(pos).is_block(blocks::TRIPWIRE_HOOK) {
            on_removed(new_state, level, pos);
            return;
        }
        emit_state(level, test_pos, attached, powered, was_attached, was_powered);
    }

    emit_state(level, pos, attached, powered, was_attached, was_powered);
    if !is_being_destroyed {
        level.set_block_simple(pos, new_state.set(HORIZONTAL_FACING, direction));
        if can_update {
            notify_neighbors(block, level, pos, direction);
        }
    }

    if was_attached != attached {
        for i in 1..receiver_pos {
            let test_pos = pos.relative_n(direction, i);
            if let Some(wire_data) = wire_states[i as usize] {
                let test_pos_state = level.block_state(test_pos);
                if test_pos_state.is_block(blocks::TRIPWIRE) || test_pos_state.is_block(blocks::TRIPWIRE_HOOK) {
                    level.set_block_simple(test_pos, wire_data.try_set(ATTACHED, attached));
                }
            }
        }
    }
}

fn emit_state(level: &mut ServerLevel, pos: BlockPos, attached: bool, powered: bool, was_attached: bool, was_powered: bool) {
    let sound = |name: &str| registry_id(registry::SOUND_EVENT, name);
    if powered && !was_powered {
        level.play_sound(None, pos, sound("minecraft:block.tripwire.click_on"), SoundSource::Blocks, 0.4, 0.6);
    } else if !powered && was_powered {
        level.play_sound(None, pos, sound("minecraft:block.tripwire.click_off"), SoundSource::Blocks, 0.4, 0.5);
    } else if attached && !was_attached {
        level.play_sound(None, pos, sound("minecraft:block.tripwire.attach"), SoundSource::Blocks, 0.4, 0.7);
    } else if !attached && was_attached {
        let pitch = 1.2 / (level.random.next_float() * 0.2 + 0.9);
        level.play_sound(None, pos, sound("minecraft:block.tripwire.detach"), SoundSource::Blocks, 0.4, pitch);
    }
}

fn notify_neighbors(block: Block, level: &mut ServerLevel, pos: BlockPos, direction: Direction) {
    let front = direction.opposite();
    level.update_neighbors_at(pos, block);
    level.update_neighbors_at(pos.relative(front), block);
}

fn on_removed(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let attached = state.get(ATTACHED);
    let powered = state.get(POWERED);
    if attached || powered {
        calculate_state(level, pos, state, true, false, -1, None);
    }
    if powered {
        notify_neighbors(state.block(), level, pos, state.get(HORIZONTAL_FACING));
    }
}

impl BlockBehaviour for TripWireHookBlock {
    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let direction = state.get(HORIZONTAL_FACING);
        let relative = pos.relative(direction.opposite());
        let block_state = level.block_state(relative);
        direction.axis().is_horizontal() && block_state.is_face_sturdy_full(direction)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if direction.opposite() == state.get(HORIZONTAL_FACING) && !super::of(state).can_survive(state, level, pos) {
            blocks::AIR.default_state()
        } else {
            state
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = block.default_state().set(POWERED, false).set(ATTACHED, false);
        let pos = ctx.clicked_pos();
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() {
                state = state.set(HORIZONTAL_FACING, direction.opposite());
                if super::of(state).can_survive(state, level, pos) {
                    return Some(state);
                }
            }
        }
        None
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        calculate_state(level, pos, state, false, false, -1, None);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        calculate_state(level, pos, state, false, true, -1, None);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        if !moved_by_piston {
            on_removed(state, level, pos);
        }
    }

    fn own_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }

    fn get_direct_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, direction: Direction) -> i32 {
        if !state.get(POWERED) {
            0
        } else if state.get(HORIZONTAL_FACING) == direction {
            15
        } else {
            0
        }
    }
}
