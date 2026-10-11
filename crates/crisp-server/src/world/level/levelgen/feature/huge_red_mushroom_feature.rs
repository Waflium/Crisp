use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::{EAST, NORTH, SOUTH, UP, WEST};

use super::abstract_huge_mushroom_feature::{AbstractHugeMushroomFeature, place_mushroom_block};
use crate::server::level::world_gen_region::WorldGenRegion;

pub fn make_cap<R: RandomSource>(feature: &AbstractHugeMushroomFeature, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos, tree_height: i32) {
    for dy in tree_height - 3..=tree_height {
        let radius = if dy < tree_height { feature.foliage_radius } else { feature.foliage_radius - 1 };
        let center = feature.foliage_radius - 2;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let min_x = dx == -radius;
                let max_x = dx == radius;
                let min_z = dz == -radius;
                let max_z = dz == radius;
                let x_edge = min_x || max_x;
                let z_edge = min_z || max_z;
                if dy >= tree_height || x_edge != z_edge {
                    let block_pos = origin.offset(dx, dy, dz);
                    let mut state = feature.cap_provider.value().get_state(level, random, origin);
                    if state.has(WEST) && state.has(EAST) && state.has(NORTH) && state.has(SOUTH) && state.has(UP) {
                        state = state.set(UP, dy >= tree_height - 1).set(WEST, dx < -center).set(EAST, dx > center).set(NORTH, dz < -center).set(SOUTH, dz > center);
                    }
                    place_mushroom_block(level, block_pos, state);
                }
            }
        }
    }
}

pub fn get_tree_radius_for_height(_trunk_height: i32, tree_height: i32, leaf_radius: i32, yo: i32) -> i32 {
    if yo < tree_height && yo >= tree_height - 3 || yo == tree_height { leaf_radius } else { 0 }
}
