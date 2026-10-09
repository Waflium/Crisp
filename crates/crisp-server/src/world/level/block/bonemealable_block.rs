use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

pub fn has_spreadable_neighbour_pos(level: &mut ServerLevel, pos: BlockPos, block_to_place: BlockState) -> bool {
    spreadable_neighbour_pos(Direction::PLANE_HORIZONTAL, level, pos, block_to_place).is_some()
}

pub fn find_spreadable_neighbour_pos(level: &mut ServerLevel, pos: BlockPos, block_to_place: BlockState) -> Option<BlockPos> {
    let mut directions = Direction::PLANE_HORIZONTAL;
    for i in (2..=directions.len()).rev() {
        let swap_to = level.random.next_int_bounded(i as i32) as usize;
        directions.swap(i - 1, swap_to);
    }
    spreadable_neighbour_pos(directions, level, pos, block_to_place)
}

fn spreadable_neighbour_pos(directions: [Direction; 4], level: &mut ServerLevel, pos: BlockPos, block_to_place: BlockState) -> Option<BlockPos> {
    for direction in directions {
        let neighbour_pos = pos.relative(direction);
        if level.block_state(neighbour_pos).is_air() && super::of(block_to_place).can_survive(block_to_place, level, neighbour_pos) {
            return Some(neighbour_pos);
        }
    }
    None
}
