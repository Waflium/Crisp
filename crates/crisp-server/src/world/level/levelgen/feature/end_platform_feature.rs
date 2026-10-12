use crisp_core::BlockPos;
use crisp_data::blocks;

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub fn create_end_platform(new_level: &mut ServerLevel, origin: BlockPos, drop_resources: bool) {
    for dz in -2..=2 {
        for dx in -2..=2 {
            for dy in -1..3 {
                let block_pos = origin.offset(dx, dy, dz);
                let block = if dy == -1 { blocks::OBSIDIAN } else { blocks::AIR };
                if !new_level.block_state(block_pos).is_block(block) {
                    if drop_resources {
                        new_level.destroy_block(block_pos, true, None, 512);
                    }
                    new_level.set_block(block_pos, block.default_state(), UPDATE_ALL);
                }
            }
        }
    }
}
