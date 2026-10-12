use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos};
use crisp_data::properties::AXIS;
use serde_json::Value;

use super::foliage_placer::{FoliageAttachment, try_place_leaf};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::{IntProvider, float};
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub struct PoplarFoliagePlacer {
    pub height: IntProvider,
    side_hole_chance: f32,
}

impl PoplarFoliagePlacer {
    pub fn parse(json: &Value) -> Result<PoplarFoliagePlacer, String> {
        Ok(PoplarFoliagePlacer {
            height: IntProvider::parse(json.get("height").ok_or("missing height")?)?,
            side_hole_chance: float(json.get("side_hole_chance").ok_or("missing side_hole_chance")?)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn replace_leaves_with_log<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        foliage_setter: &mut TreeSetter,
        tree: &TreeFeature,
        random: &mut R,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        foliage_height: i32,
        flip_rhombus_shape: bool,
    ) {
        let offset = if double_trunk { 1 } else { 0 };
        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                let abs_dz = dz.abs();
                let abs_dx = dx.abs();
                if is_within_rhombus_shape(
                    current_radius,
                    abs_dx,
                    abs_dz,
                    self.get_corner_blocks_to_cut_for_rhombus_shape(dx, dz, current_radius, self.should_row_be_partial_rhombus_shape(foliage_height, y), flip_rhombus_shape),
                    2,
                ) && (abs_dz == 0 && current_radius - abs_dx >= 4 || abs_dx == 0 && current_radius - abs_dz >= 4)
                {
                    let axis = if abs_dz == 0 { Axis::X } else { Axis::Z };
                    try_place_log(level, foliage_setter, random, tree, origin.offset(dx, y, dz), axis);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_leaves_row<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        foliage_setter: &mut TreeSetter,
        random: &mut R,
        tree: &TreeFeature,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        foliage_height: i32,
        flip_rhombus_shape: bool,
    ) {
        let offset = if double_trunk { 1 } else { 0 };
        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                if !self.should_skip_location(random, dx, y, dz, current_radius, foliage_height, flip_rhombus_shape) {
                    try_place_leaf(level, foliage_setter, random, tree, origin.offset(dx, y, dz));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn should_skip_location(&self, random: &mut impl RandomSource, dx: i32, y: i32, dz: i32, current_radius: i32, foliage_height: i32, flip_rhombus_shape: bool) -> bool {
        let should_row_be_partial_rhombus_shape = self.should_row_be_partial_rhombus_shape(foliage_height, y);
        let corner_blocks_to_cut_for_rhombus_shape = self.get_corner_blocks_to_cut_for_rhombus_shape(dx, dz, current_radius, should_row_be_partial_rhombus_shape, flip_rhombus_shape);
        let abs_dx = dx.abs();
        let abs_dz = dz.abs();
        let is_rhombus_edge_block = abs_dx == current_radius || abs_dz == current_radius;
        if should_row_be_partial_rhombus_shape && is_rhombus_edge_block {
            return true;
        }
        let additional_side_removal = if random.next_float() <= self.side_hole_chance { 1 } else { 0 };
        !is_within_rhombus_shape(current_radius, abs_dx, abs_dz, corner_blocks_to_cut_for_rhombus_shape, additional_side_removal)
    }

    fn get_corner_blocks_to_cut_for_rhombus_shape(&self, dx: i32, dz: i32, current_radius: i32, should_row_be_partial_rhombus_shape: bool, flip_rhombus_shape: bool) -> i32 {
        let is_small_corner_of_shape = if flip_rhombus_shape { is_left_top_corner_or_right_lower_corner(dx, dz) } else { is_left_lower_corner_or_right_top_corner(dx, dz) };
        if is_small_corner_of_shape {
            current_radius - 1
        } else if should_row_be_partial_rhombus_shape {
            current_radius + 1
        } else {
            current_radius
        }
    }

    fn should_row_be_partial_rhombus_shape(&self, foliage_height: i32, y: i32) -> bool {
        foliage_height - 1 == y || foliage_height - 2 == y
    }
}

fn try_place_log<R: RandomSource>(level: &mut impl WorldGenLevel, foliage_setter: &mut TreeSetter, random: &mut R, tree: &TreeFeature, pos: BlockPos, axis: Axis) {
    let current = level.get_block_state(pos);
    if current == tree.foliage_provider.value().get_state(level, random, pos) {
        let state = tree.trunk_provider.value().get_state(level, random, pos).try_set(AXIS, axis);
        foliage_setter.accept(level, pos, state);
    }
}

fn is_within_rhombus_shape(current_radius: i32, abs_dx: i32, abs_dz: i32, corner_blocks_to_cut_for_rhombus_shape: i32, additional_side_removal: i32) -> bool {
    abs_dx + abs_dz <= current_radius * 2 - (corner_blocks_to_cut_for_rhombus_shape + additional_side_removal)
}

fn is_left_lower_corner_or_right_top_corner(dx: i32, dz: i32) -> bool {
    dx > 0 && dz < 0 || dz > 0 && dx < 0
}

fn is_left_top_corner_or_right_lower_corner(dx: i32, dz: i32) -> bool {
    dx > 0 && dz > 0 || dz < 0 && dx < 0
}

#[allow(clippy::too_many_arguments)]
pub fn create_foliage<R: RandomSource>(
    poplar: &PoplarFoliagePlacer,
    level: &mut impl WorldGenLevel,
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
    let flip_rhombus_shape = random.next_boolean();
    let h = foliage_height + foliage_attachment.foliage_height_offset;
    poplar.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius - 2, h - 1, double_trunk, h, flip_rhombus_shape);
    poplar.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius - 1, h - 2, double_trunk, h, flip_rhombus_shape);
    poplar.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius - 1, h - 3, double_trunk, h, flip_rhombus_shape);
    let mut y = h - 4;
    while y >= 1 {
        poplar.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius, y, double_trunk, h, flip_rhombus_shape);
        y -= 1;
    }
    poplar.replace_leaves_with_log(level, foliage_setter, tree, random, foliage_pos, current_radius, h - 4, double_trunk, h, flip_rhombus_shape);
    poplar.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, current_radius - 1, 0, double_trunk, h, flip_rhombus_shape);
    poplar.place_leaves_row(level, foliage_setter, random, tree, foliage_pos, (current_radius - 2).clamp(1, 2), -1, double_trunk, h, flip_rhombus_shape);
}
