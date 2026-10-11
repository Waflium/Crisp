use crisp_core::random::RandomSource;
use serde_json::Value;

use super::foliage_placer::{FoliageAttachment, FoliagePlacer};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::{IntProvider, float};
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub struct CherryFoliagePlacer {
    pub height: IntProvider,
    wide_bottom_layer_hole_chance: f32,
    corner_hole_chance: f32,
    hanging_leaves_chance: f32,
    hanging_leaves_extension_chance: f32,
}

impl CherryFoliagePlacer {
    pub fn parse(json: &Value) -> Result<CherryFoliagePlacer, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(CherryFoliagePlacer {
            height: IntProvider::parse(field("height")?)?,
            wide_bottom_layer_hole_chance: float(field("wide_bottom_layer_hole_chance")?)?,
            corner_hole_chance: float(field("corner_hole_chance")?)?,
            hanging_leaves_chance: float(field("hanging_leaves_chance")?)?,
            hanging_leaves_extension_chance: float(field("hanging_leaves_extension_chance")?)?,
        })
    }

    pub fn should_skip_location(&self, random: &mut impl RandomSource, dx: i32, y: i32, dz: i32, current_radius: i32) -> bool {
        if y == -1 && (dx == current_radius || dz == current_radius) && random.next_float() < self.wide_bottom_layer_hole_chance {
            return true;
        }
        let corner = dx == current_radius && dz == current_radius;
        let wide_layer = current_radius > 2;
        if wide_layer {
            corner || dx + dz > current_radius * 2 - 2 && random.next_float() < self.corner_hole_chance
        } else {
            corner && random.next_float() < self.corner_hole_chance
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_foliage<R: RandomSource>(
    cherry: &CherryFoliagePlacer,
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
    let current_radius = leaf_radius + foliage_attachment.radius_offset_xz - 1;
    let foliage_height_with_offset = foliage_height + foliage_attachment.foliage_height_offset;
    placer.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius - 2, foliage_height_with_offset - 3, double_trunk);
    placer.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius - 1, foliage_height_with_offset - 4, double_trunk);
    let mut y = foliage_height_with_offset - 5;
    while y >= 0 {
        placer.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius, y, double_trunk);
        y -= 1;
    }
    placer.place_leaves_row_with_hanging_leaves_below(
        level,
        foliage_setter,
        random,
        tree,
        foliage_pos,
        current_radius,
        -1,
        double_trunk,
        cherry.hanging_leaves_chance,
        cherry.hanging_leaves_extension_chance,
    );
    placer.place_leaves_row_with_hanging_leaves_below(
        level,
        foliage_setter,
        random,
        tree,
        foliage_pos,
        current_radius - 1,
        -2,
        double_trunk,
        cherry.hanging_leaves_chance,
        cherry.hanging_leaves_extension_chance,
    );
}
