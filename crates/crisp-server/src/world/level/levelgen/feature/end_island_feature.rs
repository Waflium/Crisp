use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use crisp_data::blocks;

use crate::world::level::world_gen_level::WorldGenLevel;

pub fn place<R: RandomSource>(level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
    let mut size = random.next_int_bounded(3) as f32 + 4.0;
    let mut y = 0;
    while size > 0.5 {
        for x in mth::floor_f(-size)..=mth::ceil(size as f64) {
            for z in mth::floor_f(-size)..=mth::ceil(size as f64) {
                if (x * x + z * z) as f32 <= (size + 1.0) * (size + 1.0) {
                    level.set_block_and_update(origin.offset(x, y, z), blocks::END_STONE.default_state());
                }
            }
        }
        size -= random.next_int_bounded(2) as f32 + 0.5;
        y -= 1;
    }
    true
}
