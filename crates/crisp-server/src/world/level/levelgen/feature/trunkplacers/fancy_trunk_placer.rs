use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, mth};
use crisp_data::properties::AXIS;

use super::trunk_placer::{TrunkPlacer, place_below_trunk_block};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

struct FoliageCoords {
    attachment: FoliageAttachment,
    branch_base: i32,
}

impl FoliageCoords {
    fn new(pos: BlockPos, branch_base: i32) -> FoliageCoords {
        FoliageCoords { attachment: FoliageAttachment::new(pos, 0, false), branch_base }
    }

    fn get_branch_base(&self) -> i32 {
        self.branch_base
    }
}

pub fn place_trunk<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    let height = tree_height + 2;
    let trunk_height = mth::floor(height as f64 * 0.618);
    place_below_trunk_block(level, trunk_setter, random, origin.below(), tree);
    let clusters_per_y = 1.min(mth::floor(1.382 + (1.0 * height as f64 / 13.0).powf(2.0)));
    let trunk_top = origin.y + trunk_height;
    let mut relative_y = height - 5;
    let mut foliage_coords = vec![FoliageCoords::new(origin.offset(0, relative_y, 0), trunk_top)];
    while relative_y >= 0 {
        let tree_shape = tree_shape(height, relative_y);
        if tree_shape >= 0.0 {
            for _ in 0..clusters_per_y {
                let radius = 1.0 * tree_shape as f64 * (random.next_float() as f64 + 0.328);
                let angle = (random.next_float() * 2.0) as f64 * std::f64::consts::PI;
                let x = radius * angle.sin() + 0.5;
                let z = radius * angle.cos() + 0.5;
                let check_start = origin.offset(mth::floor(x), relative_y - 1, mth::floor(z));
                let check_end = check_start.offset(0, 5, 0);
                if make_limb(placer, level, trunk_setter, random, check_start, check_end, false, tree) {
                    let dx = origin.x - check_start.x;
                    let dz = origin.z - check_start.z;
                    let branch_height = check_start.y as f64 - ((dx * dx + dz * dz) as f64).sqrt() * 0.381;
                    let branch_top = if branch_height > trunk_top as f64 { trunk_top } else { branch_height as i32 };
                    let check_branch_base = BlockPos::new(origin.x, branch_top, origin.z);
                    if make_limb(placer, level, trunk_setter, random, check_branch_base, check_start, false, tree) {
                        foliage_coords.push(FoliageCoords::new(check_start, check_branch_base.y));
                    }
                }
            }
        }
        relative_y -= 1;
    }
    make_limb(placer, level, trunk_setter, random, origin, origin.offset(0, trunk_height, 0), true, tree);
    make_branches(placer, level, trunk_setter, random, height, origin, &foliage_coords, tree);
    let mut attachments = Vec::new();
    for foliage_coord in &foliage_coords {
        if trim_branches(height, foliage_coord.get_branch_base() - origin.y) {
            attachments.push(foliage_coord.attachment);
        }
    }
    attachments
}

#[allow(clippy::too_many_arguments)]
fn make_limb<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    start_pos: BlockPos,
    end_pos: BlockPos,
    do_place: bool,
    tree: &TreeFeature,
) -> bool {
    if !do_place && start_pos == end_pos {
        return true;
    }
    let delta = end_pos.offset(-start_pos.x, -start_pos.y, -start_pos.z);
    let steps = get_steps(delta);
    let dx = delta.x as f32 / steps as f32;
    let dy = delta.y as f32 / steps as f32;
    let dz = delta.z as f32 / steps as f32;
    for i in 0..=steps {
        let block_pos = start_pos.offset(mth::floor_f(0.5 + i as f32 * dx), mth::floor_f(0.5 + i as f32 * dy), mth::floor_f(0.5 + i as f32 * dz));
        if do_place {
            let axis = get_log_axis(start_pos, block_pos);
            placer.place_log_with(level, trunk_setter, random, block_pos, tree, |state| state.try_set(AXIS, axis));
        } else if !placer.is_free(level, block_pos) {
            return false;
        }
    }
    true
}

fn get_steps(pos: BlockPos) -> i32 {
    pos.x.abs().max(pos.y.abs().max(pos.z.abs()))
}

fn get_log_axis(start_pos: BlockPos, block_pos: BlockPos) -> Axis {
    let mut axis = Axis::Y;
    let xdiff = (block_pos.x - start_pos.x).abs();
    let zdiff = (block_pos.z - start_pos.z).abs();
    let maxdiff = xdiff.max(zdiff);
    if maxdiff > 0 {
        axis = if xdiff == maxdiff { Axis::X } else { Axis::Z };
    }
    axis
}

fn trim_branches(height: i32, local_y: i32) -> bool {
    local_y as f64 >= height as f64 * 0.2
}

#[allow(clippy::too_many_arguments)]
fn make_branches<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut impl WorldGenLevel,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    height: i32,
    origin: BlockPos,
    foliage_coords: &[FoliageCoords],
    tree: &TreeFeature,
) {
    for end_coord in foliage_coords {
        let branch_base = end_coord.get_branch_base();
        let base_coord = BlockPos::new(origin.x, branch_base, origin.z);
        if base_coord != end_coord.attachment.pos && trim_branches(height, branch_base - origin.y) {
            make_limb(placer, level, trunk_setter, random, base_coord, end_coord.attachment.pos, true, tree);
        }
    }
}

fn tree_shape(height: i32, y: i32) -> f32 {
    if (y as f32) < height as f32 * 0.3 {
        return -1.0;
    }
    let radius = height as f32 / 2.0;
    let adjacent = radius - y as f32;
    let mut distance = mth::sqrt(radius * radius - adjacent * adjacent);
    if adjacent == 0.0 {
        distance = radius;
    } else if adjacent.abs() >= radius {
        return 0.0;
    }
    distance * 0.5
}
