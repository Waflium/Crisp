use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState, blocks};

use super::redstone_torch_block::{self, RedstoneTorchBlock};
use super::{BlockBehaviour, wall_torch_block};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;

pub struct RedstoneWallTorchBlock {
    pub torch: RedstoneTorchBlock,
}

fn has_neighbor_signal(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
    let opposite = state.get(HORIZONTAL_FACING).opposite();
    level.has_signal(pos.relative(opposite), opposite)
}

impl BlockBehaviour for RedstoneWallTorchBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let state = super::behaviour(blocks::WALL_TORCH).get_state_for_placement(blocks::WALL_TORCH, ctx, level)?;
        Some(block.default_state().set(HORIZONTAL_FACING, state.get(HORIZONTAL_FACING)))
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        wall_torch_block::can_survive_facing(level, pos, state.get(HORIZONTAL_FACING))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        wall_torch_block::update_shape(state, level, pos, direction)
    }

    fn on_place(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        self.torch.on_place(level, pos);
    }

    fn affect_neighbors_after_removal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        self.torch.affect_neighbors_after_removal(level, pos, moved_by_piston);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let neighbor_signal = has_neighbor_signal(state, level, pos);
        self.torch.tick_with_signal(state, level, pos, neighbor_signal);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let neighbor_signal = has_neighbor_signal(state, level, pos);
        self.torch.neighbor_changed_with_signal(state, level, pos, neighbor_signal);
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        redstone_torch_block::get_direct_signal(state, level, pos, direction)
    }

    fn own_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        redstone_torch_block::own_signal(state)
    }

    fn get_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, direction: Direction) -> i32 {
        if state.get(HORIZONTAL_FACING) != direction { redstone_torch_block::own_signal(state) } else { 0 }
    }
}
