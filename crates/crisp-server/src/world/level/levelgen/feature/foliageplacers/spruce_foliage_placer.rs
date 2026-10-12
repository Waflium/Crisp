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
    let foliage_pos = foliage_attachment.pos;
    let mut current_radius = random.next_int_bounded(2);
    let mut max_radius = 1;
    let mut min_radius = 0;
    let foliage_height_with_offset = foliage_height + foliage_attachment.foliage_height_offset;
    let mut yo = offset;
    while yo >= -foliage_height_with_offset {
        placer.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius, yo, foliage_attachment.double_trunk());
        if current_radius >= max_radius {
            current_radius = min_radius;
            min_radius = 1;
            max_radius = (max_radius + 1).min(leaf_radius + foliage_attachment.radius_offset_xz);
        } else {
            current_radius += 1;
        }
        yo -= 1;
    }
}
