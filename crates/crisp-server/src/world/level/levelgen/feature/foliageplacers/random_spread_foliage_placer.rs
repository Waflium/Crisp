use crisp_core::random::RandomSource;
use serde_json::Value;

use super::foliage_placer::{FoliageAttachment, try_place_leaf};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub struct RandomSpreadFoliagePlacer {
    pub foliage_height: IntProvider,
    leaf_placement_attempts: i32,
}

impl RandomSpreadFoliagePlacer {
    pub fn parse(json: &Value) -> Result<RandomSpreadFoliagePlacer, String> {
        Ok(RandomSpreadFoliagePlacer {
            foliage_height: IntProvider::parse(json.get("foliage_height").ok_or("missing foliage_height")?)?,
            leaf_placement_attempts: json.get("leaf_placement_attempts").and_then(Value::as_i64).ok_or("missing leaf_placement_attempts")? as i32,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_foliage<R: RandomSource>(
    placer: &RandomSpreadFoliagePlacer,
    level: &mut impl WorldGenLevel,
    foliage_setter: &mut TreeSetter,
    random: &mut R,
    tree: &TreeFeature,
    foliage_attachment: &FoliageAttachment,
    foliage_height: i32,
    leaf_radius: i32,
) {
    let origin = foliage_attachment.pos;
    for _ in 0..placer.leaf_placement_attempts {
        let dx = random.next_int_bounded(leaf_radius) - random.next_int_bounded(leaf_radius);
        let dy = random.next_int_bounded(foliage_height) - random.next_int_bounded(foliage_height);
        let dz = random.next_int_bounded(leaf_radius) - random.next_int_bounded(leaf_radius);
        try_place_leaf(level, foliage_setter, random, tree, origin.offset(dx, dy, dz));
    }
}
