use crisp_core::{BlockPos, Direction};
use crisp_data::properties::POWER;
use crisp_data::{Block, BlockState};

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::block::redstone_wire_block;

pub fn get_block_signal(level: &mut ServerLevel, pos: BlockPos) -> i32 {
    redstone_wire_block::get_block_signal(level, pos)
}

pub fn get_wire_signal(wire_block: Block, state: BlockState) -> i32 {
    if state.is_block(wire_block) { state.get(POWER) } else { 0 }
}

pub fn get_incoming_wire_signal(wire_block: Block, level: &mut ServerLevel, pos: BlockPos) -> i32 {
    let mut wire_signal = 0;
    for direction in Direction::PLANE_HORIZONTAL {
        let neighbor_pos = pos.relative(direction);
        let neighbor_state = level.block_state(neighbor_pos);
        wire_signal = wire_signal.max(get_wire_signal(wire_block, neighbor_state));
        let above_pos = pos.above();
        if neighbor_state.is_redstone_conductor() && !level.block_state(above_pos).is_redstone_conductor() {
            let above_neighbor_pos = neighbor_pos.above();
            wire_signal = wire_signal.max(get_wire_signal(wire_block, level.block_state(above_neighbor_pos)));
        } else if !neighbor_state.is_redstone_conductor() {
            let below_neighbor_pos = neighbor_pos.below();
            wire_signal = wire_signal.max(get_wire_signal(wire_block, level.block_state(below_neighbor_pos)));
        }
    }
    0.max(wire_signal - 1)
}
