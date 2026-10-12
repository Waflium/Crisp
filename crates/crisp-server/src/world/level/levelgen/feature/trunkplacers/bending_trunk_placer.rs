use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use super::trunk_placer::{TrunkPlacer, place_below_trunk_block};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{self, TreeFeature, TreeSetter};

pub struct BendingTrunkPlacer {
    min_height_for_leaves: i32,
    bend_length: IntProvider,
}

impl BendingTrunkPlacer {
    pub fn parse(json: &Value) -> Result<BendingTrunkPlacer, String> {
        Ok(BendingTrunkPlacer {
            min_height_for_leaves: json.get("min_height_for_leaves").and_then(Value::as_i64).map_or(1, |v| v as i32),
            bend_length: IntProvider::parse(json.get("bend_length").ok_or("missing bend_length")?)?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn place_trunk<R: RandomSource>(
    bending: &BendingTrunkPlacer,
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    let direction = Direction::get_random_horizontal_direction(random);
    let log_height = tree_height - 1;
    let mut pos = origin;
    let below_pos = pos.below();
    place_below_trunk_block(level, trunk_setter, random, below_pos, tree);
    let mut foliage_points = Vec::new();
    for i in 0..=log_height {
        if i + 1 >= log_height + random.next_int_bounded(2) {
            pos = pos.relative(direction);
        }
        if tree_feature::valid_tree_pos(level, pos) {
            placer.place_log(level, trunk_setter, random, pos, tree);
        }
        if i >= bending.min_height_for_leaves {
            foliage_points.push(FoliageAttachment::new(pos, 0, false));
        }
        pos = pos.above();
    }
    let dir_length = bending.bend_length.sample(random);
    for _ in 0..=dir_length {
        if tree_feature::valid_tree_pos(level, pos) {
            placer.place_log(level, trunk_setter, random, pos, tree);
        }
        foliage_points.push(FoliageAttachment::new(pos, 0, false));
        pos = pos.relative(direction);
    }
    foliage_points
}
