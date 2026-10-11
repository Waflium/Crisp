use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AXIS;
use serde_json::Value;

use super::trunk_placer::{TrunkPlacer, place_below_trunk_block};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub struct CherryTrunkPlacer {
    branch_count: IntProvider,
    branch_horizontal_length: IntProvider,
    branch_start_offset_from_top: IntProvider,
    second_branch_start_offset_from_top: IntProvider,
    branch_end_offset_from_top: IntProvider,
}

impl CherryTrunkPlacer {
    pub fn parse(json: &Value) -> Result<CherryTrunkPlacer, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let branch_start = field("branch_start_offset_from_top")?;
        let bound = |key: &str| branch_start.get(key).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| format!("branch_start_offset_from_top without {key}"));
        let (min_inclusive, max_inclusive) = (bound("min_inclusive")?, bound("max_inclusive")?);
        let branch_start_offset_from_top = IntProvider::Uniform { min_inclusive, max_inclusive };
        Ok(CherryTrunkPlacer {
            branch_count: IntProvider::parse(field("branch_count")?)?,
            branch_horizontal_length: IntProvider::parse(field("branch_horizontal_length")?)?,
            branch_start_offset_from_top,
            second_branch_start_offset_from_top: IntProvider::Uniform { min_inclusive, max_inclusive: max_inclusive - 1 },
            branch_end_offset_from_top: IntProvider::parse(field("branch_end_offset_from_top")?)?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn place_trunk<R: RandomSource>(
    cherry: &CherryTrunkPlacer,
    placer: &TrunkPlacer,
    level: &mut WorldGenRegion,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    place_below_trunk_block(level, trunk_setter, random, origin.below(), tree);
    let first_branch_offset_from_origin = 0.max(tree_height - 1 + cherry.branch_start_offset_from_top.sample(random));
    let mut second_branch_offset_from_origin = 0.max(tree_height - 1 + cherry.second_branch_start_offset_from_top.sample(random));
    if second_branch_offset_from_origin >= first_branch_offset_from_origin {
        second_branch_offset_from_origin += 1;
    }
    let branch_count = cherry.branch_count.sample(random);
    let has_middle_branch = branch_count == 3;
    let has_both_side_branches = branch_count >= 2;
    let trunk_height = if has_middle_branch {
        tree_height
    } else if has_both_side_branches {
        first_branch_offset_from_origin.max(second_branch_offset_from_origin) + 1
    } else {
        first_branch_offset_from_origin + 1
    };
    for y in 0..trunk_height {
        placer.place_log(level, trunk_setter, random, origin.offset(0, y, 0), tree);
    }
    let mut attachments = Vec::new();
    if has_middle_branch {
        attachments.push(FoliageAttachment::new(origin.offset(0, trunk_height, 0), 0, false));
    }
    let tree_direction = Direction::get_random_horizontal_direction(random);
    attachments.push(generate_branch(
        cherry,
        placer,
        level,
        trunk_setter,
        random,
        tree_height,
        origin,
        tree,
        tree_direction,
        first_branch_offset_from_origin,
        first_branch_offset_from_origin < trunk_height - 1,
    ));
    if has_both_side_branches {
        attachments.push(generate_branch(
            cherry,
            placer,
            level,
            trunk_setter,
            random,
            tree_height,
            origin,
            tree,
            tree_direction.opposite(),
            second_branch_offset_from_origin,
            second_branch_offset_from_origin < trunk_height - 1,
        ));
    }
    attachments
}

#[allow(clippy::too_many_arguments)]
fn generate_branch<R: RandomSource>(
    cherry: &CherryTrunkPlacer,
    placer: &TrunkPlacer,
    level: &mut WorldGenRegion,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
    branch_direction: Direction,
    offset_from_origin: i32,
    middle_continues_upwards: bool,
) -> FoliageAttachment {
    let sideways_axis = branch_direction.axis();
    let mut log_pos = origin.offset(0, offset_from_origin, 0);
    let branch_end_pos_offset_from_origin = tree_height - 1 + cherry.branch_end_offset_from_top.sample(random);
    let extend_branch_away_from_trunk = middle_continues_upwards || branch_end_pos_offset_from_origin < offset_from_origin;
    let distance_to_trunk = cherry.branch_horizontal_length.sample(random) + if extend_branch_away_from_trunk { 1 } else { 0 };
    let branch_end_pos = origin.relative_n(branch_direction, distance_to_trunk).offset(0, branch_end_pos_offset_from_origin, 0);
    let steps_horizontally = if extend_branch_away_from_trunk { 2 } else { 1 };
    for _ in 0..steps_horizontally {
        log_pos = log_pos.relative(branch_direction);
        placer.place_log_with(level, trunk_setter, random, log_pos, tree, |state| state.try_set(AXIS, sideways_axis));
    }
    let vertical_direction = if branch_end_pos.y > log_pos.y { Direction::Up } else { Direction::Down };
    loop {
        let distance = log_pos.dist_manhattan(branch_end_pos);
        if distance == 0 {
            return FoliageAttachment::new(branch_end_pos.above(), 0, false);
        }
        let chance_to_grow_vertically = (branch_end_pos.y - log_pos.y).abs() as f32 / distance as f32;
        let grow_vertically = random.next_float() < chance_to_grow_vertically;
        log_pos = log_pos.relative(if grow_vertically { vertical_direction } else { branch_direction });
        if grow_vertically {
            placer.place_log(level, trunk_setter, random, log_pos, tree);
        } else {
            placer.place_log_with(level, trunk_setter, random, log_pos, tree, |state| state.try_set(AXIS, sideways_axis));
        }
    }
}
