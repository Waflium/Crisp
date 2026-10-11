use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{AGE_2, HORIZONTAL_FACING};
use crisp_data::{Block, BlockState, block_tags, blocks};

use super::BlockBehaviour;
use super::horizontal_directional_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct CocoaBlock;

impl BlockBehaviour for CocoaBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        horizontal_directional_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        horizontal_directional_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = block.default_state();
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() {
                state = state.set(HORIZONTAL_FACING, direction);
                if self.can_survive(state, level, ctx.clicked_pos()) {
                    return Some(state);
                }
            }
        }
        None
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == state.get(HORIZONTAL_FACING) && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.relative(state.get(HORIZONTAL_FACING))).is(&block_tags::SUPPORTS_COCOA)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.random.next_int_bounded(5) == 0 {
            let age = state.get(AGE_2);
            if age < 2 {
                level.set_block(pos, state.set(AGE_2, age + 1), UPDATE_CLIENTS);
            }
        }
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        state.get(AGE_2) < 2
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        level.set_block(pos, state.set(AGE_2, state.get(AGE_2) + 1), UPDATE_CLIENTS);
    }
}
