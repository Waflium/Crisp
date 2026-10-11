use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::rotated_pillar_block::RotatedPillarBlock;
use super::rotation::Rotation;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::LevelAccessor;

pub struct ChainBlock;

impl BlockBehaviour for ChainBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        RotatedPillarBlock.rotate(state, rotation)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let water = is_water_source(level, ctx.clicked_pos());
        RotatedPillarBlock.get_state_for_placement(block, ctx, level).map(|state| state.set(WATERLOGGED, water))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }
}
