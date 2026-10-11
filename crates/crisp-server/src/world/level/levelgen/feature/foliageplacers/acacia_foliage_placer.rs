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
    let double_trunk = foliage_attachment.double_trunk();
    let foliage_pos = foliage_attachment.pos.offset(0, offset, 0);
    let foliage_height_with_offset = foliage_height + foliage_attachment.foliage_height_offset;
    placer.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, leaf_radius + foliage_attachment.radius_offset_xz, -1 - foliage_height_with_offset, double_trunk);
    placer.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, leaf_radius - 1, -foliage_height_with_offset, double_trunk);
    placer.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, leaf_radius + foliage_attachment.radius_offset_xz - 1, 0, double_trunk);
}

pub fn should_skip_location(dx: i32, y: i32, dz: i32, current_radius: i32) -> bool {
    if y == 0 { (dx > 1 || dz > 1) && dx != 0 && dz != 0 } else { dx == current_radius && dz == current_radius && current_radius > 0 }
}
