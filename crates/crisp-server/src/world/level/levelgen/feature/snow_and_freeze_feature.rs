use crisp_core::BlockPos;
use crisp_data::blocks;
use crisp_data::properties::SNOWY;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::chunk::HeightmapType;

pub fn place(level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
    for dx in 0..16 {
        for dz in 0..16 {
            let x = origin.x + dx;
            let z = origin.z + dz;
            let y = level.get_height(HeightmapType::MotionBlocking, x, z);
            let top_pos = BlockPos::new(x, y, z);
            let below_pos = top_pos.below();
            let biome = level.get_biome(top_pos);
            if biome.should_freeze(level, below_pos, false) {
                level.set_block(below_pos, blocks::ICE.default_state(), 2);
            }
            if biome.should_snow(level, top_pos) {
                level.set_block(top_pos, blocks::SNOW.default_state(), 2);
                let below_state = level.get_block_state(below_pos);
                if below_state.has(SNOWY) {
                    level.set_block(below_pos, below_state.set(SNOWY, true), 2);
                }
            }
        }
    }
    true
}
