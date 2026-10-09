use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{FACING, POWERED, WATERLOGGED};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_ALL;

const ACTIVATION_TICKS: i32 = 8;

pub struct LightningRodBlock;

fn update_neighbours(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let front = state.get(FACING).opposite();
    level.update_neighbors_at(pos.relative(front), state.block());
}

impl BlockBehaviour for LightningRodBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let water = is_water_source(level, ctx.clicked_pos());
        Some(block.default_state().set(FACING, ctx.clicked_face()).set(WATERLOGGED, water))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn own_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }

    fn get_direct_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, direction: Direction) -> i32 {
        if state.get(POWERED) && state.get(FACING) == direction { 15 } else { 0 }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        level.set_block(pos, state.set(POWERED, false), UPDATE_ALL);
        update_neighbours(state, level, pos);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        if state.get(POWERED) {
            update_neighbours(state, level, pos);
        }
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !state.is_block(old_state.block()) && state.get(POWERED) && !level.has_scheduled_tick(pos, state.block()) {
            level.schedule_tick(pos, state.block(), ACTIVATION_TICKS);
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction};
    use crisp_data::blocks;
    use crisp_data::properties::{FACING, POWERED};

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn power_runs_out_after_eight_ticks() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::LIGHTNING_ROD.default_state().set(POWERED, true), UPDATE_ALL);
        assert_eq!(level.get_signal(pos, Direction::East), 15);
        assert_eq!(level.get_direct_signal(pos, Direction::Up), 15);
        assert_eq!(level.get_direct_signal(pos, Direction::Down), 0);
        assert!(level.has_scheduled_tick(pos, blocks::LIGHTNING_ROD));
        let state = level.block_state(pos);
        crate::world::level::block::of(state).tick(state, &mut level, pos);
        assert!(!level.block_state(pos).get(POWERED));
        assert_eq!(level.block_state(pos).get(FACING), Direction::Up);
    }
}
