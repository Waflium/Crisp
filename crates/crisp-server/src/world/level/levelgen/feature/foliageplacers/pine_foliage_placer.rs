use crisp_core::random::RandomSource;

use super::foliage_placer::{FoliageAttachment, FoliagePlacer};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

#[allow(clippy::too_many_arguments)]
pub fn create_foliage<R: RandomSource>(
    placer: &FoliagePlacer,
    level: &mut WorldGenRegion,
    foliage_setter: &mut TreeSetter,
    random: &mut R,
    tree: &TreeFeature,
    foliage_attachment: &FoliageAttachment,
    foliage_height: i32,
    leaf_radius: i32,
    offset: i32,
) {
    let mut current_radius = 0;
    let foliage_height_with_offset = foliage_height + foliage_attachment.foliage_height_offset;
    let mut yo = offset;
    while yo >= offset - foliage_height_with_offset {
        placer.place_leaves_row(level, foliage_setter, random, tree, foliage_attachment.pos, current_radius, yo, foliage_attachment.double_trunk());
        if current_radius >= 1 && yo == offset - foliage_height_with_offset + 1 {
            current_radius -= 1;
        } else if current_radius < leaf_radius + foliage_attachment.radius_offset_xz {
            current_radius += 1;
        }
        yo -= 1;
    }
}
