use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::random::RandomSource;
use crisp_data::properties::{EAST, NORTH, SOUTH, WEST};

use super::tree_decorator::Context;

pub fn place<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>) {
    for i in 0..context.logs.len() {
        let pos = context.logs[i];
        if context.random.next_int_bounded(3) > 0 {
            let west = pos.west();
            if context.is_air(west) {
                context.place_vine(west, EAST);
            }
        }
        if context.random.next_int_bounded(3) > 0 {
            let east = pos.east();
            if context.is_air(east) {
                context.place_vine(east, WEST);
            }
        }
        if context.random.next_int_bounded(3) > 0 {
            let north = pos.north();
            if context.is_air(north) {
                context.place_vine(north, SOUTH);
            }
        }
        if context.random.next_int_bounded(3) > 0 {
            let south = pos.south();
            if context.is_air(south) {
                context.place_vine(south, NORTH);
            }
        }
    }
}
