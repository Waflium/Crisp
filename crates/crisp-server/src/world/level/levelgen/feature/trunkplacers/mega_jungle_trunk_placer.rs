use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};

use super::giant_trunk_placer;
use super::trunk_placer::TrunkPlacer;
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
    let mut attachments = giant_trunk_placer::place_trunk(placer, level, trunk_setter, random, tree_height, origin, tree);
    let mut branch_height = tree_height - 2 - random.next_int_bounded(4);
    while branch_height > tree_height / 2 {
        let angle = random.next_float() * (std::f64::consts::PI * 2.0) as f32;
        let mut bx = 0;
        let mut bz = 0;
        for b in 0..5 {
            bx = (1.5 + mth::cos(angle as f64) * b as f32) as i32;
            bz = (1.5 + mth::sin(angle as f64) * b as f32) as i32;
            let pos = origin.offset(bx, branch_height - 3 + b / 2, bz);
            placer.place_log(level, trunk_setter, random, pos, tree);
        }
        attachments.push(FoliageAttachment::new(origin.offset(bx, branch_height, bz), -2, false));
        branch_height -= 2 + random.next_int_bounded(4);
    }
    attachments
}
