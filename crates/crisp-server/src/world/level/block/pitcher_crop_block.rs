use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{AGE_4, DOUBLE_BLOCK_HALF, DoubleBlockHalf};
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::crop_block;
use super::double_plant_block;
use super::vegetation_block::Vegetation;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS};

pub struct PitcherCropBlock;

fn is_lower(state: BlockState) -> bool {
    state.is_block(blocks::PITCHER_CROP) && state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower
}

fn is_double(age: i32) -> bool {
    age >= 3
}

fn is_max_age(state: BlockState) -> bool {
    state.get(AGE_4) >= 4
}

fn can_grow_into(level: &mut ServerLevel, pos: BlockPos) -> bool {
    let state = level.block_state(pos);
    state.is_air() || state.is_block(blocks::PITCHER_CROP)
}

fn can_grow(level: &mut ServerLevel, lower_pos: BlockPos, lower_state: BlockState, new_age: i32) -> bool {
    !is_max_age(lower_state)
        && crop_block::has_sufficient_light(level, lower_pos)
        && !level.chunks.is_outside_build_height(lower_pos.y + 1)
        && (!is_double(new_age) || can_grow_into(level, lower_pos.above()))
}

fn grow(level: &mut ServerLevel, lower_state: BlockState, lower_pos: BlockPos, increase: i32) {
    let updated_age = (lower_state.get(AGE_4) + increase).min(4);
    if can_grow(level, lower_pos, lower_state, updated_age) {
        let new_lower = lower_state.set(AGE_4, updated_age);
        level.set_block(lower_pos, new_lower, UPDATE_CLIENTS);
        if is_double(updated_age) {
            level.set_block(lower_pos.above(), new_lower.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper), UPDATE_ALL);
        }
    }
}

fn lower_half(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> Option<(BlockPos, BlockState)> {
    if is_lower(state) {
        return Some((pos, state));
    }
    let lower_pos = pos.below();
    let lower_state = level.block_state(lower_pos);
    is_lower(lower_state).then_some((lower_pos, lower_state))
}

impl Vegetation for PitcherCropBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> bool {
        crop_block::may_place_on_crop_support(state)
    }
}

impl BlockBehaviour for PitcherCropBlock {
    fn get_state_for_placement(&self, block: Block, _ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state())
    }

    fn can_be_replaced(&self, _state: BlockState, _ctx: &BlockPlaceContext) -> bool {
        false
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        if is_double(state.get(AGE_4)) {
            double_plant_block::update_shape(state, level, pos, direction, neighbour_state)
        } else if self.can_survive(state, level, pos) {
            state
        } else {
            blocks::AIR.default_state()
        }
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        if is_lower(state) && !crop_block::has_sufficient_light(level, pos) {
            return false;
        }
        double_plant_block::can_survive(self, state, level, pos)
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        double_plant_block::player_will_destroy(level, pos, state, player)
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, _state: BlockState, _block_entity: Option<&crate::world::level::block::entity::BlockEntity>, tool: Option<ItemStack>) {
        double_plant_block::player_destroy(level, player, pos, tool);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let growth_speed = crop_block::growth_speed(state.block(), level, pos);
        let should_progress_growth = level.random.next_int_bounded((25.0f32 / growth_speed) as i32 + 1) == 0;
        if should_progress_growth {
            grow(level, state, pos, 1);
        }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        match lower_half(level, pos, state) {
            Some((lower_pos, lower_state)) => can_grow(level, lower_pos, lower_state, lower_state.get(AGE_4) + 1),
            None => false,
        }
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if let Some((lower_pos, lower_state)) = lower_half(level, pos, state) {
            grow(level, lower_state, lower_pos, 1);
        }
    }
}
