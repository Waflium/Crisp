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
    place_below_trunk_block(level, trunk_setter, random, origin.below(), tree);
    for y in 0..tree_height {
        placer.place_log(level, trunk_setter, random, origin.offset(0, y, 0), tree);
    }
    vec![FoliageAttachment::new(origin.offset(0, tree_height, 0), 0, false)]
}
