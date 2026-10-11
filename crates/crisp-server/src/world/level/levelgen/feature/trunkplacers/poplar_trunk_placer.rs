use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AXIS;
use serde_json::Value;

use super::trunk_placer::{TrunkPlacer, place_below_trunk_block};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub struct PoplarTrunkPlacer {
    trunk_height_above_branches: IntProvider,
    branch_amount: IntProvider,
}

impl PoplarTrunkPlacer {
    pub fn parse(json: &Value) -> Result<PoplarTrunkPlacer, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(PoplarTrunkPlacer {
            trunk_height_above_branches: IntProvider::parse(field("trunk_height_above_branches")?)?,
            branch_amount: IntProvider::parse(field("branch_amount")?)?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn place_trunk<R: RandomSource>(
    poplar: &PoplarTrunkPlacer,
    placer: &TrunkPlacer,
    level: &mut WorldGenRegion,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    place_below_trunk_block(level, trunk_setter, random, origin.below(), tree);
    let trunk_height_up_to_foliage_branches = tree_height - poplar.trunk_height_above_branches.sample(random);
    for y in 0..tree_height {
        placer.place_log(level, trunk_setter, random, origin.offset(0, y, 0), tree);
        let directions = get_shuffled_branch_directions(random);
        if trunk_height_up_to_foliage_branches - 1 == y {
            let branches = poplar.branch_amount.sample(random);
            for x in 0..branches {
                let branch_direction = directions[x as usize];
                let axis = branch_direction.axis();
                placer.place_log_with(level, trunk_setter, random, origin.offset(0, y, 0).relative(branch_direction), tree, |state| state.try_set(AXIS, axis));
            }
        }
    }
    vec![FoliageAttachment::new(origin.offset(0, trunk_height_up_to_foliage_branches, 0), 0, false)]
}

fn get_shuffled_branch_directions(random: &mut impl RandomSource) -> Vec<Direction> {
    Direction::all_shuffled(random).into_iter().filter(|direction| !direction.axis().is_vertical()).collect()
}
