use crisp_core::{BlockPos, ChunkPos};
use crisp_data::blocks;

use crate::world::level::world_gen_level::WorldGenLevel;

const PLATFORM_OFFSET: BlockPos = BlockPos::new(8, 3, 8);
const PLATFORM_RADIUS: i32 = 16;
const PLATFORM_RADIUS_CHUNKS: i32 = 1;

fn checkerboard_distance(xa: i32, za: i32, xb: i32, zb: i32) -> i32 {
    (xa - xb).abs().max((za - zb).abs())
}

pub fn place(level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
    let platform_origin_chunk = PLATFORM_OFFSET.chunk();
    let current_chunk_pos = origin.chunk();
    if checkerboard_distance(current_chunk_pos.x, current_chunk_pos.z, platform_origin_chunk.x, platform_origin_chunk.z) > PLATFORM_RADIUS_CHUNKS {
        return true;
    }
    let platform_origin = BlockPos::new(PLATFORM_OFFSET.x, origin.y + PLATFORM_OFFSET.y, PLATFORM_OFFSET.z);
    let chunk = ChunkPos::new(current_chunk_pos.x, current_chunk_pos.z);
    for z in chunk.min_block_z()..=chunk.min_block_z() + 15 {
        for x in chunk.min_block_x()..=chunk.min_block_x() + 15 {
            if checkerboard_distance(platform_origin.x, platform_origin.z, x, z) <= PLATFORM_RADIUS {
                let block_pos = BlockPos::new(x, platform_origin.y, z);
                if block_pos == platform_origin {
                    level.set_block(block_pos, blocks::COBBLESTONE.default_state(), 2);
                } else {
                    level.set_block(block_pos, blocks::STONE.default_state(), 2);
                }
            }
        }
    }
    true
}
