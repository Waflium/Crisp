use crisp_core::random::RandomSource;
use crisp_data::BlockState;

use crate::server::level::ServerLevel;

pub fn is_valid_growth_state(state: BlockState) -> bool {
    state.is_air()
}

pub fn blocks_to_grow_when_bonemealed(level: &mut ServerLevel) -> i32 {
    let mut grow_probability = 1.0;
    let mut count = 0;
    while level.random.next_double() < grow_probability {
        grow_probability *= 0.826;
        count += 1;
    }
    count
}
