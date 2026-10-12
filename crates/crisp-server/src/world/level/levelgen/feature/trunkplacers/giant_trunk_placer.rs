use crisp_core::BlockPos;
use crisp_core::random::RandomSource;

use super::trunk_placer::{TrunkPlacer, place_below_trunk_block};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub fn place_trunk<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    let below = origin.below();
    place_below_trunk_block(level, trunk_setter, random, below, tree);
    place_below_trunk_block(level, trunk_setter, random, below.east(), tree);
    place_below_trunk_block(level, trunk_setter, random, below.south(), tree);
    place_below_trunk_block(level, trunk_setter, random, below.south().east(), tree);
    for hh in 0..tree_height {
        place_log_if_free_with_offset(placer, level, trunk_setter, random, tree, origin, 0, hh, 0);
        if hh < tree_height - 1 {
            place_log_if_free_with_offset(placer, level, trunk_setter, random, tree, origin, 1, hh, 0);
            place_log_if_free_with_offset(placer, level, trunk_setter, random, tree, origin, 1, hh, 1);
            place_log_if_free_with_offset(placer, level, trunk_setter, random, tree, origin, 0, hh, 1);
        }
    }
    vec![FoliageAttachment::new(origin.offset(0, tree_height, 0), 0, true)]
}

#[allow(clippy::too_many_arguments)]
fn place_log_if_free_with_offset<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree: &TreeFeature,
    tree_pos: BlockPos,
    x: i32,
    y: i32,
    z: i32,
) {
    placer.place_log_if_free(level, trunk_setter, random, tree_pos.offset(x, y, z), tree);
}
