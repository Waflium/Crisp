use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{FLOWER_AMOUNT, HORIZONTAL_FACING};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::segmentable_block;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct FlowerBedBlock;

impl Vegetation for FlowerBedBlock {}

impl BlockBehaviour for FlowerBedBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(segmentable_block::get_state_for_placement(ctx, level, block, FLOWER_AMOUNT))
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        segmentable_block::can_be_replaced(state, ctx, FLOWER_AMOUNT) || vegetation_block::block_can_be_replaced(state, ctx)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let amount = state.get(FLOWER_AMOUNT);
        if amount < 4 {
            level.set_block(pos, state.set(FLOWER_AMOUNT, amount + 1), UPDATE_CLIENTS);
        } else {
            level.pop_resource(pos, ItemStack::new(state.block().item(), 1));
        }
    }
}
