use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::{HORIZONTAL_FACING, TILT, Tilt, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, FluidState, block_tags, blocks, registry, registry_id};

use super::BlockBehaviour;
use super::big_dripleaf_stem_block;
use super::vegetation_block::schedule_water_tick;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS};

pub struct BigDripleafBlock;

fn delay_until_next_tilt_state(tilt: Tilt) -> Option<i32> {
    match tilt {
        Tilt::None => None,
        Tilt::Unstable | Tilt::Partial => Some(10),
        Tilt::Full => Some(100),
    }
}

pub fn place_with_random_height(level: &mut ServerLevel, stem_bottom_pos: BlockPos, facing: Direction) {
    let desired_height = mth::next_int(&mut level.random, 2, 5);
    let mut pos = stem_bottom_pos;
    let mut height = 0;
    while height < desired_height && can_place_at(level, pos) {
        height += 1;
        pos = pos.above();
    }
    let leaf_y = stem_bottom_pos.y + height - 1;
    pos.y = stem_bottom_pos.y;
    while pos.y < leaf_y {
        let fluid = level.fluid_state(pos);
        big_dripleaf_stem_block::place(level, pos, fluid, facing);
        pos = pos.above();
    }
    let fluid = level.fluid_state(pos);
    place(level, pos, fluid, facing);
}

fn can_replace(old_state: BlockState) -> bool {
    old_state.is_air() || old_state.is_block(blocks::WATER) || old_state.is_block(blocks::SMALL_DRIPLEAF)
}

pub fn can_place_at(level: &mut ServerLevel, pos: BlockPos) -> bool {
    let state = level.block_state(pos);
    !level.chunks.is_outside_build_height(pos.y) && can_replace(state)
}

pub fn place(level: &mut ServerLevel, pos: BlockPos, fluid: FluidState, facing: Direction) -> bool {
    let state = blocks::BIG_DRIPLEAF.default_state().set(WATERLOGGED, fluid.is_source_of_type(Fluid::Water)).set(HORIZONTAL_FACING, facing);
    level.set_block(pos, state, UPDATE_ALL)
}

fn play_tilt_sound(level: &mut ServerLevel, pos: BlockPos, sound: &str) {
    let pitch = mth::random_between(&mut level.random, 0.8, 1.2);
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, sound), SoundSource::Blocks, 1.0, pitch);
}

fn set_tilt_and_schedule_tick(state: BlockState, level: &mut ServerLevel, pos: BlockPos, tilt: Tilt, sound: Option<&str>) {
    set_tilt(state, level, pos, tilt);
    if let Some(sound) = sound {
        play_tilt_sound(level, pos, sound);
    }
    if let Some(delay) = delay_until_next_tilt_state(tilt) {
        level.schedule_tick(pos, state.block(), delay);
    }
}

fn reset_tilt(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    set_tilt(state, level, pos, Tilt::None);
    if state.get(TILT) != Tilt::None {
        play_tilt_sound(level, pos, "minecraft:block.big_dripleaf.tilt_up");
    }
}

fn set_tilt(state: BlockState, level: &mut ServerLevel, pos: BlockPos, tilt: Tilt) {
    level.set_block(pos, state.set(TILT, tilt), UPDATE_CLIENTS);
}

impl BlockBehaviour for BigDripleafBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let below_state = level.block_state(ctx.clicked_pos().below());
        let fluid = level.fluid_state(ctx.clicked_pos());
        let below_is_dripleaf_part = below_state.is_block(blocks::BIG_DRIPLEAF) || below_state.is_block(blocks::BIG_DRIPLEAF_STEM);
        let facing = if below_is_dripleaf_part { below_state.get(HORIZONTAL_FACING) } else { ctx.horizontal_direction().opposite() };
        Some(block.default_state().set(WATERLOGGED, fluid.is_source_of_type(Fluid::Water)).set(HORIZONTAL_FACING, facing))
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let below_state = level.block_state(pos.below());
        below_state.is_block(state.block()) || below_state.is_block(blocks::BIG_DRIPLEAF_STEM) || below_state.is(&block_tags::SUPPORTS_BIG_DRIPLEAF)
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        if direction == Direction::Down && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        if state.get(WATERLOGGED) {
            schedule_water_tick(level, pos);
        }
        if direction == Direction::Up && neighbour_state.is_block(state.block()) {
            blocks::BIG_DRIPLEAF_STEM.default_state().set(WATERLOGGED, state.get(WATERLOGGED)).set(HORIZONTAL_FACING, state.get(HORIZONTAL_FACING))
        } else {
            state
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.has_neighbor_signal(pos) {
            reset_tilt(state, level, pos);
            return;
        }
        match state.get(TILT) {
            Tilt::Unstable => set_tilt_and_schedule_tick(state, level, pos, Tilt::Partial, Some("minecraft:block.big_dripleaf.tilt_down")),
            Tilt::Partial => set_tilt_and_schedule_tick(state, level, pos, Tilt::Full, Some("minecraft:block.big_dripleaf.tilt_down")),
            Tilt::Full => reset_tilt(state, level, pos),
            Tilt::None => {}
        }
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        if level.has_neighbor_signal(pos) {
            reset_tilt(state, level, pos);
        }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        can_place_at(level, pos.above())
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let above_pos = pos.above();
        if can_place_at(level, above_pos) {
            let facing = state.get(HORIZONTAL_FACING);
            big_dripleaf_stem_block::place(level, pos, state.fluid_state(), facing);
            let above_fluid = level.block_state(above_pos).fluid_state();
            place(level, above_pos, above_fluid, facing);
        }
    }
}
