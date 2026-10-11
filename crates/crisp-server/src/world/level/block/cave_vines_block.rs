use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{AGE_25, BERRIES};
use crisp_data::{Block, BlockState, blocks};

use super::growing_plant_head_block::{GrowingPlantHeadBlock, HeadParts};
use super::{BlockBehaviour, BlockHitResult, cave_vines};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct CaveVinesParts;

impl HeadParts for CaveVinesParts {
    fn body_block(&self) -> Block {
        blocks::CAVE_VINES_PLANT
    }

    fn can_grow_into(&self, state: BlockState) -> bool {
        state.is_air()
    }

    fn blocks_to_grow_when_bonemealed(&self, _level: &mut ServerLevel) -> i32 {
        1
    }

    fn grow_into_state(&self, grow_from_state: BlockState, level: &mut ServerLevel) -> BlockState {
        grow_from_state.cycle(AGE_25).set(BERRIES, level.random.next_float() < 0.11)
    }

    fn update_body_after_converted_from_head(&self, head_state: BlockState, body_state: BlockState) -> BlockState {
        body_state.set(BERRIES, head_state.get(BERRIES))
    }
}

pub struct CaveVinesBlock(pub GrowingPlantHeadBlock<CaveVinesParts>);

impl BlockBehaviour for CaveVinesBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        self.0.get_state_for_placement(block, ctx, level)
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        self.0.can_survive(state, level, pos)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        self.0.tick(state, level, pos);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        self.0.random_tick(state, level, pos);
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        self.0.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        cave_vines::use_vine(Some(player), state, level, pos)
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        !state.get(BERRIES)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        level.set_block(pos, state.set(BERRIES, true), UPDATE_CLIENTS);
    }
}
