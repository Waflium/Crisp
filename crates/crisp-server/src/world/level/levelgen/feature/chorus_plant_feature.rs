use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::block_tags;

use crate::world::level::block::chorus_flower_block;
use crate::world::level::world_gen_level::WorldGenLevel;

pub fn place<R: RandomSource>(level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
    if level.get_block_state(origin).is_air() && level.get_block_state(origin.below()).is(&block_tags::SUPPORTS_CHORUS_PLANT) {
        chorus_flower_block::generate_plant(level, origin, random, 8);
        true
    } else {
        false
    }
}
