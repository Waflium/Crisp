use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::leaves_block;
use super::mangrove_propagule_block::new_hanging_propagule;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct MangroveLeavesBlock;

impl BlockBehaviour for MangroveLeavesBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(leaves_block::get_state_for_placement(block, ctx, level))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        leaves_block::update_shape(state, level, pos, neighbour_state)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        leaves_block::tick(state, level, pos);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        leaves_block::random_tick(state, level, pos);
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        level.block_state(pos.below()).is_air()
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        level.set_block(pos.below(), new_hanging_propagule(0), UPDATE_CLIENTS);
    }
}
