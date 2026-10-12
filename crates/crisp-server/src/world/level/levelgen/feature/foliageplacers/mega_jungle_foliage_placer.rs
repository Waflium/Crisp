use crisp_core::random::RandomSource;

use super::foliage_placer::{FoliageAttachment, FoliagePlacer};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

#[allow(clippy::too_many_arguments)]
pub fn create_foliage<R: RandomSource>(
    placer: &FoliagePlacer,
    level: &mut impl WorldGenLevel,
    foliage_setter: &mut TreeSetter,
    random: &mut R,
    tree: &TreeFeature,
    foliage_attachment: &FoliageAttachment,
    foliage_height: i32,
    leaf_radius: i32,
    offset: i32,
) {
    let leaf_height = (if foliage_attachment.double_trunk() { foliage_height } else { 1 + random.next_int_bounded(2) }) + foliage_attachment.foliage_height_offset;
    let mut yo = offset;
    while yo >= offset - leaf_height {
        let current_radius = leaf_radius + foliage_attachment.radius_offset_xz + 1 - yo;
        placer.place_leaves_row(level, foliage_setter, random, tree, foliage_attachment.pos, current_radius, yo, foliage_attachment.double_trunk());
        yo -= 1;
    }
}
