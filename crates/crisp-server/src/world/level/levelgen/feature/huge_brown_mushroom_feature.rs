use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::{EAST, NORTH, SOUTH, WEST};

use super::abstract_huge_mushroom_feature::{AbstractHugeMushroomFeature, place_mushroom_block};
use crate::server::level::world_gen_region::WorldGenRegion;

pub fn make_cap<R: RandomSource>(feature: &AbstractHugeMushroomFeature, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos, tree_height: i32) {
    let r = feature.foliage_radius;
    for dx in -r..=r {
        for dz in -r..=r {
            let min_x = dx == -r;
            let max_x = dx == r;
            let min_z = dz == -r;
            let max_z = dz == r;
            let x_edge = min_x || max_x;
            let z_edge = min_z || max_z;
            if !x_edge || !z_edge {
                let block_pos = origin.offset(dx, tree_height, dz);
                let west = min_x || z_edge && dx == 1 - r;
                let east = max_x || z_edge && dx == r - 1;
                let north = min_z || x_edge && dz == 1 - r;
                let south = max_z || x_edge && dz == r - 1;
                let mut state = feature.cap_provider.value().get_state(level, random, origin);
                if state.has(WEST) && state.has(EAST) && state.has(NORTH) && state.has(SOUTH) {
                    state = state.set(WEST, west).set(EAST, east).set(NORTH, north).set(SOUTH, south);
                }
                place_mushroom_block(level, block_pos, state);
            }
        }
    }
}

pub fn get_tree_radius_for_height(leaf_radius: i32, yo: i32) -> i32 {
    if yo <= 3 { 0 } else { leaf_radius }
}
