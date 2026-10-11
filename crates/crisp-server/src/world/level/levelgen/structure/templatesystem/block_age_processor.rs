use crisp_core::Direction;
use crisp_core::random::RandomSource;
use crisp_data::properties::{HALF, HORIZONTAL_FACING, Half};
use crisp_data::{Block, BlockState, block_tags, blocks};

use super::structure_place_settings::StructurePlaceSettings;
use super::structure_template::StructureBlockInfo;

const PROBABILITY_OF_REPLACING_FULL_BLOCK: f32 = 0.5;
const PROBABILITY_OF_REPLACING_STAIRS: f32 = 0.5;
const PROBABILITY_OF_REPLACING_OBSIDIAN: f32 = 0.15;

fn non_mossy_replacements() -> [BlockState; 2] {
    [blocks::STONE_SLAB.default_state(), blocks::STONE_BRICK_SLAB.default_state()]
}

pub fn process_block<R: RandomSource>(mossiness: f32, processed_block_info: StructureBlockInfo, settings: &StructurePlaceSettings, random: &mut R) -> Option<StructureBlockInfo> {
    let mut random = settings.get_random(processed_block_info.pos, random);
    let state = processed_block_info.state;
    let new_state = if state.is_block(blocks::STONE_BRICKS) || state.is_block(blocks::STONE) || state.is_block(blocks::CHISELED_STONE_BRICKS) {
        maybe_replace_full_stone_block(mossiness, &mut random)
    } else if state.is(&block_tags::STAIRS) {
        maybe_replace_stairs(mossiness, state, &mut random)
    } else if state.is(&block_tags::SLABS) {
        maybe_replace_slab(mossiness, state, &mut random)
    } else if state.is(&block_tags::WALLS) {
        maybe_replace_wall(mossiness, state, &mut random)
    } else if state.is_block(blocks::OBSIDIAN) {
        maybe_replace_obsidian(&mut random)
    } else {
        None
    };
    match new_state {
        Some(new_state) => Some(StructureBlockInfo { pos: processed_block_info.pos, state: new_state, nbt: processed_block_info.nbt }),
        None => Some(processed_block_info),
    }
}

fn maybe_replace_full_stone_block(mossiness: f32, random: &mut impl RandomSource) -> Option<BlockState> {
    if random.next_float() >= PROBABILITY_OF_REPLACING_FULL_BLOCK {
        return None;
    }
    let non_mossy_replacements = [blocks::CRACKED_STONE_BRICKS.default_state(), get_random_facing_stairs(random, blocks::STONE_BRICK_STAIRS)];
    let mossy_replacements = [blocks::MOSSY_STONE_BRICKS.default_state(), get_random_facing_stairs(random, blocks::MOSSY_STONE_BRICK_STAIRS)];
    Some(get_random_block(mossiness, random, &non_mossy_replacements, &mossy_replacements))
}

fn maybe_replace_stairs(mossiness: f32, block_state: BlockState, random: &mut impl RandomSource) -> Option<BlockState> {
    if random.next_float() >= PROBABILITY_OF_REPLACING_STAIRS {
        return None;
    }
    let mossy_replacements = [blocks::MOSSY_STONE_BRICK_STAIRS.default_state().with_properties_of(block_state), blocks::MOSSY_STONE_BRICK_SLAB.default_state()];
    Some(get_random_block(mossiness, random, &non_mossy_replacements(), &mossy_replacements))
}

fn maybe_replace_slab(mossiness: f32, block_state: BlockState, random: &mut impl RandomSource) -> Option<BlockState> {
    (random.next_float() < mossiness).then(|| blocks::MOSSY_STONE_BRICK_SLAB.default_state().with_properties_of(block_state))
}

fn maybe_replace_wall(mossiness: f32, block_state: BlockState, random: &mut impl RandomSource) -> Option<BlockState> {
    (random.next_float() < mossiness).then(|| blocks::MOSSY_STONE_BRICK_WALL.default_state().with_properties_of(block_state))
}

fn maybe_replace_obsidian(random: &mut impl RandomSource) -> Option<BlockState> {
    (random.next_float() < PROBABILITY_OF_REPLACING_OBSIDIAN).then(|| blocks::CRYING_OBSIDIAN.default_state())
}

fn get_random_facing_stairs(random: &mut impl RandomSource, stair_block: Block) -> BlockState {
    let facing = Direction::get_random_horizontal_direction(random);
    let half = if random.next_int_bounded(2) == 0 { Half::Top } else { Half::Bottom };
    stair_block.default_state().set(HORIZONTAL_FACING, facing).set(HALF, half)
}

fn get_random_block(mossiness: f32, random: &mut impl RandomSource, non_mossy_blocks: &[BlockState; 2], mossy_blocks: &[BlockState; 2]) -> BlockState {
    if random.next_float() < mossiness { get_random_block_of(random, mossy_blocks) } else { get_random_block_of(random, non_mossy_blocks) }
}

fn get_random_block_of(random: &mut impl RandomSource, blocks: &[BlockState; 2]) -> BlockState {
    blocks[random.next_int_bounded(blocks.len() as i32) as usize]
}
