use crisp_core::BlockPos;
use crisp_data::{BlockState, blocks};

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

pub fn find_blocks(
    level: &mut ServerLevel,
    from: BlockPos,
    to: BlockPos,
    predicate: &dyn Fn(BlockState) -> bool,
    consumer: &mut dyn FnMut(&mut ServerLevel, BlockPos, BlockState) -> bool,
) -> bool {
    let (min_x, min_y, min_z) = (from.x.min(to.x), from.y.min(to.y), from.z.min(to.z));
    let (max_x, max_y, max_z) = (from.x.max(to.x), from.y.max(to.y), from.z.max(to.z));
    section_based_scan(level, min_x, min_y, min_z, max_x, max_y, max_z, predicate, consumer)
}

#[allow(clippy::too_many_arguments)]
fn section_based_scan(
    level: &mut ServerLevel,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
    predicate: &dyn Fn(BlockState) -> bool,
    consumer: &mut dyn FnMut(&mut ServerLevel, BlockPos, BlockState) -> bool,
) -> bool {
    let level_min_y = level.chunks.min_y();
    let level_max_y = level_min_y + level.chunks.height() - 1;
    let min_section_x = min_x >> 4;
    let min_section_y = (level_min_y >> 4).max(min_y >> 4);
    let min_section_z = min_z >> 4;
    let max_section_x = max_x >> 4;
    let max_section_y = (level_max_y >> 4).min(max_y >> 4);
    let max_section_z = max_z >> 4;
    for x in min_section_x..=max_section_x {
        for z in min_section_z..=max_section_z {
            for y in min_section_y..=max_section_y {
                let (section_origin_x, section_origin_y, section_origin_z) = (x << 4, y << 4, z << 4);
                let from_x = section_origin_x.max(min_x) & 15;
                let from_y = section_origin_y.max(min_y) & 15;
                let from_z = section_origin_z.max(min_z) & 15;
                let to_x = (section_origin_x + 15).min(max_x) & 15;
                let to_y = (section_origin_y + 15).min(max_y) & 15;
                let to_z = (section_origin_z + 15).min(max_z) & 15;
                let origin = BlockPos::new(section_origin_x, section_origin_y, section_origin_z);
                if find_blocks_in_section(level, origin, from_x, from_y, from_z, to_x, to_y, to_z, predicate, consumer) {
                    return true;
                }
            }
        }
    }
    let below_world = min_y < level_min_y;
    let above_world = max_y > level_max_y;
    let air = blocks::AIR.default_state();
    if (below_world || above_world) && predicate(air) {
        if below_world {
            for pos in BlockPos::between_closed(BlockPos::new(min_x, min_y, min_z), BlockPos::new(max_x, level_min_y - 1, max_z)) {
                if consumer(level, pos, air) {
                    return true;
                }
            }
        }
        if above_world {
            for pos in BlockPos::between_closed(BlockPos::new(min_x, level_max_y + 1, min_z), BlockPos::new(max_x, max_y, max_z)) {
                if consumer(level, pos, air) {
                    return true;
                }
            }
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn find_blocks_in_section(
    level: &mut ServerLevel,
    origin: BlockPos,
    from_x: i32,
    from_y: i32,
    from_z: i32,
    to_x: i32,
    to_y: i32,
    to_z: i32,
    predicate: &dyn Fn(BlockState) -> bool,
    consumer: &mut dyn FnMut(&mut ServerLevel, BlockPos, BlockState) -> bool,
) -> bool {
    for y in from_y..=to_y {
        for z in from_z..=to_z {
            for x in from_x..=to_x {
                let pos = origin.offset(x, y, z);
                let block_state = level.block_state(pos);
                if predicate(block_state) && consumer(level, pos, block_state) {
                    return true;
                }
            }
        }
    }
    false
}
