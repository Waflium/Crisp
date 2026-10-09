use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::lighting::light_dampening_into;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct NyliumBlock;

fn can_be_nylium(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
    let above_state = level.block_state(pos.above());
    light_dampening_into(state, above_state, Direction::Up, above_state.light_dampening() as i32) < 15
}

impl BlockBehaviour for NyliumBlock {
    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !can_be_nylium(state, level, pos) {
            level.set_block(pos, blocks::NETHERRACK.default_state(), UPDATE_ALL);
        }
    }
}
