use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::Direction;
use crisp_core::random::RandomSource;
use crisp_data::blocks;
use crisp_data::properties::{AGE_2, HORIZONTAL_FACING};

use super::tree_decorator::Context;

pub fn place<R: RandomSource>(probability: f32, context: &mut Context<impl WorldGenLevel, R>) {
    if context.random.next_float() >= probability {
        return;
    }
    if context.logs.is_empty() {
        return;
    }
    let tree_y = context.logs[0].y;
    for i in 0..context.logs.len() {
        let pos = context.logs[i];
        if pos.y - tree_y > 2 {
            continue;
        }
        for direction in Direction::PLANE_HORIZONTAL {
            if context.random.next_float() <= 0.25 {
                let opposite = direction.opposite();
                let cocoa_pos = pos.offset(opposite.step_x(), 0, opposite.step_z());
                if context.is_air(cocoa_pos) {
                    let age = context.random.next_int_bounded(3);
                    context.set_block(cocoa_pos, blocks::COCOA.default_state().set(AGE_2, age).set(HORIZONTAL_FACING, direction));
                }
            }
        }
    }
}
