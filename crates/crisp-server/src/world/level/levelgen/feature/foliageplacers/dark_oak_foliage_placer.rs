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
    leaf_radius: i32,
    offset: i32,
) {
    let pos = foliage_attachment.pos.offset(0, offset, 0);
    let double_trunk = foliage_attachment.double_trunk();
    if double_trunk {
        placer.place_leaves_row(level, foliage_setter, random, tree, pos, leaf_radius + 2, -1, double_trunk);
        placer.place_leaves_row(level, foliage_setter, random, tree, pos, leaf_radius + 3, 0, double_trunk);
        placer.place_leaves_row(level, foliage_setter, random, tree, pos, leaf_radius + 2, 1, double_trunk);
        if random.next_boolean() {
            placer.place_leaves_row(level, foliage_setter, random, tree, pos, leaf_radius, 2, double_trunk);
        }
    } else {
        placer.place_leaves_row(level, foliage_setter, random, tree, pos, leaf_radius + 2, -1, double_trunk);
        placer.place_leaves_row(level, foliage_setter, random, tree, pos, leaf_radius + 1, 0, double_trunk);
    }
}

pub fn should_skip_location(dx: i32, y: i32, dz: i32, current_radius: i32, double_trunk: bool) -> bool {
    if y == -1 && !double_trunk {
        dx == current_radius && dz == current_radius
    } else if y == 1 {
        dx + dz > current_radius * 2 - 2
    } else {
        false
    }
}
