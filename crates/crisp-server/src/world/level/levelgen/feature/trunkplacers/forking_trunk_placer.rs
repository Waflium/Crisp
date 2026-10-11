use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};

use super::trunk_placer::{TrunkPlacer, place_below_trunk_block};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub fn place_trunk<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut WorldGenRegion,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    place_below_trunk_block(level, trunk_setter, random, origin.below(), tree);
    let mut attachments = Vec::new();
    let lean_direction = Direction::get_random_horizontal_direction(random);
    let lean_height = tree_height - random.next_int_bounded(4) - 1;
    let mut lean_steps = 3 - random.next_int_bounded(3);
    let mut tx = origin.x;
    let mut tz = origin.z;
    let mut ey = None;
    for yo in 0..tree_height {
        let yy = origin.y + yo;
        if yo >= lean_height && lean_steps > 0 {
            tx += lean_direction.step_x();
            tz += lean_direction.step_z();
            lean_steps -= 1;
        }
        if placer.place_log(level, trunk_setter, random, BlockPos::new(tx, yy, tz), tree) {
            ey = Some(yy + 1);
        }
    }
    if let Some(ey) = ey {
        attachments.push(FoliageAttachment::new(BlockPos::new(tx, ey, tz), 1, false));
    }
    tx = origin.x;
    tz = origin.z;
    let branch_direction = Direction::get_random_horizontal_direction(random);
    if branch_direction != lean_direction {
        let branch_pos = lean_height - random.next_int_bounded(2) - 1;
        let mut branch_steps = 1 + random.next_int_bounded(3);
        let mut ey = None;
        let mut yo = branch_pos;
        while yo < tree_height && branch_steps > 0 {
            if yo >= 1 {
                let yy = origin.y + yo;
                tx += branch_direction.step_x();
                tz += branch_direction.step_z();
                if placer.place_log(level, trunk_setter, random, BlockPos::new(tx, yy, tz), tree) {
                    ey = Some(yy + 1);
                }
            }
            yo += 1;
            branch_steps -= 1;
        }
        if let Some(ey) = ey {
            attachments.push(FoliageAttachment::new(BlockPos::new(tx, ey, tz), 0, false));
        }
    }
    attachments
}
