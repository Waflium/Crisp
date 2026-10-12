use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};

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
    let mut prev_radius = 0;
    let foliage_height_with_offset = foliage_height + foliage_attachment.foliage_height_offset;
    for yy in foliage_pos.y - foliage_height_with_offset + offset..=foliage_pos.y + offset {
        let yo = foliage_pos.y - yy;
        let smooth_radius = leaf_radius + foliage_attachment.radius_offset_xz + mth::floor_f(yo as f32 / foliage_height_with_offset as f32 * 3.5);
        let jagged_radius = if yo > 0 && smooth_radius == prev_radius && (yy & 1) == 0 { smooth_radius + 1 } else { smooth_radius };
        placer.place_leaves_row(level, foliage_setter, random, tree, BlockPos::new(foliage_pos.x, yy, foliage_pos.z), jagged_radius, 0, foliage_attachment.double_trunk());
        prev_radius = smooth_radius;
    }
}
