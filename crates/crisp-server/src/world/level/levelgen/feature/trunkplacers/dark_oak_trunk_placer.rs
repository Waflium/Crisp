use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};

use super::trunk_placer::{TrunkPlacer, place_below_trunk_block};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{self, TreeFeature, TreeSetter};

pub fn place_trunk<R: RandomSource>(
    placer: &TrunkPlacer,
    level: &mut WorldGenRegion,
    trunk_setter: &mut TreeSetter,
    random: &mut R,
    tree_height: i32,
    origin: BlockPos,
    tree: &TreeFeature,
) -> Vec<FoliageAttachment> {
    let mut attachments = Vec::new();
    let below = origin.below();
    place_below_trunk_block(level, trunk_setter, random, below, tree);
    place_below_trunk_block(level, trunk_setter, random, below.east(), tree);
    place_below_trunk_block(level, trunk_setter, random, below.south(), tree);
    place_below_trunk_block(level, trunk_setter, random, below.south().east(), tree);
    let lean_direction = Direction::get_random_horizontal_direction(random);
    let lean_height = tree_height - random.next_int_bounded(4);
    let mut lean_steps = 2 - random.next_int_bounded(3);
    let (x, y, z) = (origin.x, origin.y, origin.z);
    let mut tx = x;
    let mut tz = z;
    let ey = y + tree_height - 1;
    for dy in 0..tree_height {
        if dy >= lean_height && lean_steps > 0 {
            tx += lean_direction.step_x();
            tz += lean_direction.step_z();
            lean_steps -= 1;
        }
        let yy = y + dy;
        let block_pos = BlockPos::new(tx, yy, tz);
        if tree_feature::is_air_or_leaves(level, block_pos) {
            placer.place_log(level, trunk_setter, random, block_pos, tree);
            placer.place_log(level, trunk_setter, random, block_pos.east(), tree);
            placer.place_log(level, trunk_setter, random, block_pos.south(), tree);
            placer.place_log(level, trunk_setter, random, block_pos.east().south(), tree);
        }
    }
    attachments.push(FoliageAttachment::new(BlockPos::new(tx, ey, tz), 0, true));
    for ox in -1..=2 {
        for oz in -1..=2 {
            if (!(0..=1).contains(&ox) || !(0..=1).contains(&oz)) && random.next_int_bounded(3) <= 0 {
                let length = random.next_int_bounded(3) + 2;
                for branch_y in 0..length {
                    placer.place_log(level, trunk_setter, random, BlockPos::new(x + ox, ey - branch_y - 1, z + oz), tree);
                }
                attachments.push(FoliageAttachment::new(BlockPos::new(x + ox, ey, z + oz), 0, false));
            }
        }
    }
    attachments
}
