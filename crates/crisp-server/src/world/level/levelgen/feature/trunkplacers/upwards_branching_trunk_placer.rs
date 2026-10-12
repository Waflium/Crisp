use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use super::trunk_placer::TrunkPlacer;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::{IntProvider, float};
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{self, TreeFeature, TreeSetter};

pub struct UpwardsBranchingTrunkPlacer {
    extra_branch_steps: IntProvider,
    place_branch_per_log_probability: f32,
    extra_branch_length: IntProvider,
    can_grow_through: BlockSet,
}

impl UpwardsBranchingTrunkPlacer {
    pub fn parse(json: &Value) -> Result<UpwardsBranchingTrunkPlacer, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(UpwardsBranchingTrunkPlacer {
            extra_branch_steps: IntProvider::parse(field("extra_branch_steps")?)?,
            place_branch_per_log_probability: float(field("place_branch_per_log_probability")?)?,
            extra_branch_length: IntProvider::parse(field("extra_branch_length")?)?,
            can_grow_through: BlockSet::parse(field("can_grow_through")?)?,
        })
    }

    pub fn valid_tree_pos(&self, level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
        tree_feature::valid_tree_pos(level, pos) || self.can_grow_through.contains(level.get_block_state(pos).block())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn place_trunk<R: RandomSource>(
    branching: &UpwardsBranchingTrunkPlacer,
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    let mut attachments = Vec::new();
    for height_pos in 0..tree_height {
        let current_height = origin.y + height_pos;
        let mut log_pos = BlockPos::new(origin.x, current_height, origin.z);
        if placer.place_log(level, trunk_setter, random, log_pos, tree) && height_pos < tree_height - 1 && random.next_float() < branching.place_branch_per_log_probability {
            let branch_dir = Direction::get_random_horizontal_direction(random);
            let branch_len = branching.extra_branch_length.sample(random);
            let branch_pos = 0.max(branch_len - branching.extra_branch_length.sample(random) - 1);
            let branch_steps = branching.extra_branch_steps.sample(random);
            place_branch(placer, level, trunk_setter, random, tree_height, tree, &mut attachments, &mut log_pos, current_height, branch_dir, branch_pos, branch_steps);
        }
        if height_pos == tree_height - 1 {
            attachments.push(FoliageAttachment::new(BlockPos::new(origin.x, current_height + 1, origin.z), 0, false));
        }
    }
    attachments
}

#[allow(clippy::too_many_arguments)]
fn place_branch<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    tree: &TreeFeature,
    attachments: &mut Vec<FoliageAttachment>,
    log_pos: &mut BlockPos,
    current_height: i32,
    branch_dir: Direction,
    branch_pos: i32,
    mut branch_steps: i32,
) {
    let mut height_along_branch = current_height + branch_pos;
    let mut log_x = log_pos.x;
    let mut log_z = log_pos.z;
    let mut branch_placement_index = branch_pos;
    while branch_placement_index < tree_height && branch_steps > 0 {
        if branch_placement_index >= 1 {
            let placement_height = current_height + branch_placement_index;
            log_x += branch_dir.step_x();
            log_z += branch_dir.step_z();
            height_along_branch = placement_height;
            *log_pos = BlockPos::new(log_x, placement_height, log_z);
            if placer.place_log(level, trunk_setter, random, *log_pos, tree) {
                height_along_branch += 1;
            }
            attachments.push(FoliageAttachment::new(*log_pos, 0, false));
        }
        branch_placement_index += 1;
        branch_steps -= 1;
    }
    if height_along_branch - current_height > 1 {
        let foliage_pos = BlockPos::new(log_x, height_along_branch, log_z);
        attachments.push(FoliageAttachment::new(foliage_pos, 0, false));
        attachments.push(FoliageAttachment::new(foliage_pos.offset(0, -2, 0), 0, false));
    }
}
