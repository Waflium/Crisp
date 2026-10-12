use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::{BooleanProperty, EAST, NORTH, SOUTH, WEST};

use super::tree_decorator::Context;

pub fn place<R: RandomSource>(probability: f32, context: &mut Context<impl WorldGenLevel, R>) {
    for i in 0..context.leaves.len() {
        let pos = context.leaves[i];
        if context.random.next_float() < probability {
            let west = pos.west();
            if context.is_air(west) {
                add_hanging_vine(west, EAST, context);
            }
        }
        if context.random.next_float() < probability {
            let east = pos.east();
            if context.is_air(east) {
                add_hanging_vine(east, WEST, context);
            }
        }
        if context.random.next_float() < probability {
            let north = pos.north();
            if context.is_air(north) {
                add_hanging_vine(north, SOUTH, context);
            }
        }
        if context.random.next_float() < probability {
            let south = pos.south();
            if context.is_air(south) {
                add_hanging_vine(south, NORTH, context);
            }
        }
    }
}

fn add_hanging_vine<R: RandomSource>(pos: BlockPos, direction: BooleanProperty, context: &mut Context<impl WorldGenLevel, R>) {
    context.place_vine(pos, direction);
    let mut max_dir = 4;
    let mut pos = pos.below();
    while context.is_air(pos) && max_dir > 0 {
        context.place_vine(pos, direction);
        pos = pos.below();
        max_dir -= 1;
    }
}
