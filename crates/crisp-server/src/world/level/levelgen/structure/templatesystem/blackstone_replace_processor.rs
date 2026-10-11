use crisp_data::properties::{HALF, HORIZONTAL_FACING, SLAB_TYPE};
use crisp_data::{Block, blocks};

use super::structure_template::StructureBlockInfo;

fn replacement(block: Block) -> Option<Block> {
    Some(match block {
        blocks::COBBLESTONE => blocks::BLACKSTONE,
        blocks::MOSSY_COBBLESTONE => blocks::BLACKSTONE,
        blocks::STONE => blocks::POLISHED_BLACKSTONE,
        blocks::STONE_BRICKS => blocks::POLISHED_BLACKSTONE_BRICKS,
        blocks::MOSSY_STONE_BRICKS => blocks::POLISHED_BLACKSTONE_BRICKS,
        blocks::COBBLESTONE_STAIRS => blocks::BLACKSTONE_STAIRS,
        blocks::MOSSY_COBBLESTONE_STAIRS => blocks::BLACKSTONE_STAIRS,
        blocks::STONE_STAIRS => blocks::POLISHED_BLACKSTONE_STAIRS,
        blocks::STONE_BRICK_STAIRS => blocks::POLISHED_BLACKSTONE_BRICK_STAIRS,
        blocks::MOSSY_STONE_BRICK_STAIRS => blocks::POLISHED_BLACKSTONE_BRICK_STAIRS,
        blocks::COBBLESTONE_SLAB => blocks::BLACKSTONE_SLAB,
        blocks::MOSSY_COBBLESTONE_SLAB => blocks::BLACKSTONE_SLAB,
        blocks::SMOOTH_STONE_SLAB => blocks::POLISHED_BLACKSTONE_SLAB,
        blocks::STONE_SLAB => blocks::POLISHED_BLACKSTONE_SLAB,
        blocks::STONE_BRICK_SLAB => blocks::POLISHED_BLACKSTONE_BRICK_SLAB,
        blocks::MOSSY_STONE_BRICK_SLAB => blocks::POLISHED_BLACKSTONE_BRICK_SLAB,
        blocks::STONE_BRICK_WALL => blocks::POLISHED_BLACKSTONE_BRICK_WALL,
        blocks::MOSSY_STONE_BRICK_WALL => blocks::POLISHED_BLACKSTONE_BRICK_WALL,
        blocks::COBBLESTONE_WALL => blocks::BLACKSTONE_WALL,
        blocks::MOSSY_COBBLESTONE_WALL => blocks::BLACKSTONE_WALL,
        blocks::CHISELED_STONE_BRICKS => blocks::CHISELED_POLISHED_BLACKSTONE,
        blocks::CRACKED_STONE_BRICKS => blocks::CRACKED_POLISHED_BLACKSTONE_BRICKS,
        blocks::IRON_BARS => blocks::IRON_CHAIN,
        _ => return None,
    })
}

pub fn process_block(processed_block_info: StructureBlockInfo) -> Option<StructureBlockInfo> {
    let Some(new_block) = replacement(processed_block_info.state.block()) else {
        return Some(processed_block_info);
    };
    let old_state = processed_block_info.state;
    let mut new_state = new_block.default_state();
    if let Some(facing) = old_state.try_get(HORIZONTAL_FACING) {
        new_state = new_state.set(HORIZONTAL_FACING, facing);
    }
    if let Some(half) = old_state.try_get(HALF) {
        new_state = new_state.set(HALF, half);
    }
    if let Some(slab_type) = old_state.try_get(SLAB_TYPE) {
        new_state = new_state.set(SLAB_TYPE, slab_type);
    }
    Some(StructureBlockInfo { pos: processed_block_info.pos, state: new_state, nbt: processed_block_info.nbt })
}
