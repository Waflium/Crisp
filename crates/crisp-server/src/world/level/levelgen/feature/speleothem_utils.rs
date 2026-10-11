use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::{SPELEOTHEM_THICKNESS, SpeleothemThickness, VERTICAL_DIRECTION, WATERLOGGED};
use crisp_data::{Block, BlockState, blocks};

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::LevelReader;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;

pub fn get_speleothem_height(mut xz_distance_from_center: f64, speleothem_radius: f64, scale: f64, bluntness: f64) -> f64 {
    if xz_distance_from_center < bluntness {
        xz_distance_from_center = bluntness;
    }
    let r = xz_distance_from_center / speleothem_radius * 0.384;
    let part1 = 0.75 * r.powf(1.3333333333333333);
    let part2 = r.powf(0.6666666666666666);
    let part3 = 0.3333333333333333 * r.ln();
    let mut height_relative_to_max_radius = scale * (part1 - part2 - part3);
    height_relative_to_max_radius = height_relative_to_max_radius.max(0.0);
    height_relative_to_max_radius / 0.384 * speleothem_radius
}

pub fn is_circle_mostly_embedded_in_stone(level: &mut WorldGenRegion, center: BlockPos, xz_radius: i32) -> bool {
    if is_empty_or_water_or_lava_at(level, center) {
        return false;
    }
    let angle_increment = 6.0 / xz_radius as f32;
    let mut angle = 0.0f32;
    while angle < (std::f64::consts::PI * 2.0) as f32 {
        let dx = (mth::cos(angle as f64) * xz_radius as f32) as i32;
        let dz = (mth::sin(angle as f64) * xz_radius as f32) as i32;
        if is_empty_or_water_or_lava_at(level, center.offset(dx, 0, dz)) {
            return false;
        }
        angle += angle_increment;
    }
    true
}

pub fn is_empty_or_water_at(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    is_empty_or_water(level.get_block_state(pos))
}

pub fn is_empty_or_water_or_lava_at(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    is_empty_or_water_or_lava(level.get_block_state(pos))
}

fn build_base_to_tip_column(direction: Direction, total_length: i32, merged_tip: bool, mut consumer: impl FnMut(BlockState), pointed_block: Block) {
    if total_length >= 3 {
        consumer(create_pointed_block(direction, SpeleothemThickness::Base, pointed_block));
        for _ in 0..total_length - 3 {
            consumer(create_pointed_block(direction, SpeleothemThickness::Middle, pointed_block));
        }
    }
    if total_length >= 2 {
        consumer(create_pointed_block(direction, SpeleothemThickness::Frustum, pointed_block));
    }
    if total_length >= 1 {
        consumer(create_pointed_block(direction, if merged_tip { SpeleothemThickness::TipMerge } else { SpeleothemThickness::Tip }, pointed_block));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn grow_speleothem(
    level: &mut WorldGenRegion,
    start_pos: BlockPos,
    tip_direction: Direction,
    height: i32,
    merged_tip: bool,
    base_block: Block,
    pointed_block: Block,
    replaceable_blocks: &BlockSet,
) {
    if is_base(level.get_block_state(start_pos.relative(tip_direction.opposite())), base_block, replaceable_blocks) {
        let mut pos = start_pos;
        build_base_to_tip_column(
            tip_direction,
            height,
            merged_tip,
            |mut state| {
                if state.is_block(pointed_block) {
                    state = state.set(WATERLOGGED, level.is_water_at(pos));
                }
                level.set_block(pos, state, 2);
                pos = pos.relative(tip_direction);
            },
            pointed_block,
        );
    }
}

pub fn place_base_block_if_possible(level: &mut WorldGenRegion, pos: BlockPos, base_block: Block, replaceable_blocks: &BlockSet) -> bool {
    let state = level.get_block_state(pos);
    if replaceable_blocks.contains(state.block()) {
        level.set_block(pos, base_block.default_state(), 2);
        true
    } else {
        false
    }
}

fn create_pointed_block(direction: Direction, thickness: SpeleothemThickness, pointed_block: Block) -> BlockState {
    pointed_block.default_state().set(VERTICAL_DIRECTION, direction).set(SPELEOTHEM_THICKNESS, thickness)
}

pub fn is_base_or_lava(state: BlockState, base_block: Block, replaceable_blocks: &BlockSet) -> bool {
    is_base(state, base_block, replaceable_blocks) || state.is_block(blocks::LAVA)
}

pub fn is_base(state: BlockState, base_block: Block, replaceable_blocks: &BlockSet) -> bool {
    state.is_block(base_block) || replaceable_blocks.contains(state.block())
}

pub fn is_empty_or_water(state: BlockState) -> bool {
    state.is_air() || state.is_block(blocks::WATER)
}

pub fn is_neither_empty_nor_water(state: BlockState) -> bool {
    !state.is_air() && !state.is_block(blocks::WATER)
}

pub fn is_empty_or_water_or_lava(state: BlockState) -> bool {
    state.is_air() || state.is_block(blocks::WATER) || state.is_block(blocks::LAVA)
}
