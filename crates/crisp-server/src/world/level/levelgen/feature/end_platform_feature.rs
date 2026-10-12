use crisp_core::BlockPos;
use crisp_data::blocks;

use crate::world::level::world_gen_level::WorldGenLevel;

pub fn place(level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
    create_end_platform(level, origin, false);
    true
}

pub fn create_end_platform(new_level: &mut impl WorldGenLevel, origin: BlockPos, drop_resources: bool) {
    for dz in -2..=2 {
        for dx in -2..=2 {
            for dy in -1..3 {
                let block_pos = origin.offset(dx, dy, dz);
                let block = if dy == -1 { blocks::OBSIDIAN } else { blocks::AIR };
                if !new_level.get_block_state(block_pos).is_block(block) {
                    if drop_resources {
                        new_level.destroy_block(block_pos, true);
                    }
                    new_level.set_block_and_update(block_pos, block.default_state());
                }
            }
        }
    }
}
