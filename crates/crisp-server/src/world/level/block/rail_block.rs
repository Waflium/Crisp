use crisp_core::{BlockPos, Direction};
use crisp_data::properties::RAIL_SHAPE;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::base_rail_block::{self as base, BaseRailBlock};
use super::mirror::Mirror;
use super::rail_state::RailState;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct RailBlock {
    pub block: Block,
}

impl BaseRailBlock for RailBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn update_state_for_neighbor(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block) {
        if block.default_state().is_signal_source() && RailState::new(pos, state).count_potential_connections(level) == 3 {
            base::update_dir(level, pos, state, false);
        }
    }
}

impl BlockBehaviour for RailBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        let current_shape = state.get(RAIL_SHAPE);
        let new_shape = base::rotate(current_shape, rotation);
        state.set(RAIL_SHAPE, new_shape)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        let current_shape = state.get(RAIL_SHAPE);
        let new_shape = base::mirror(current_shape, mirror);
        state.set(RAIL_SHAPE, new_shape)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(base::get_state_for_placement(block, ctx, level))
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        base::can_survive(level, pos)
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {
        base::on_place(self, state, level, pos, old_state, moved_by_piston);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        base::neighbor_changed(self, state, level, pos, block, moved_by_piston);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        base::affect_neighbors_after_removal(self, state, level, pos, moved_by_piston);
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
        base::update_shape(state, level, pos)
    }
}
