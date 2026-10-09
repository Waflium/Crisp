use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{DISTANCE, PERSISTENT, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::schedule_water_tick;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub fn update_distance(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> BlockState {
    let mut new_distance = 7;
    for direction in Direction::VALUES {
        new_distance = new_distance.min(distance_at(level.block_state(pos.relative(direction))) + 1);
        if new_distance == 1 {
            break;
        }
    }
    state.set(DISTANCE, new_distance)
}

fn distance_at(state: BlockState) -> i32 {
    optional_distance_at(state).unwrap_or(7)
}

pub fn optional_distance_at(state: BlockState) -> Option<i32> {
    if state.is(&block_tags::PREVENTS_NEARBY_LEAF_DECAY) { Some(0) } else { state.try_get(DISTANCE) }
}

pub fn random_tick(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if decaying(state) {
        level.drop_resources(state, pos, None, None);
        level.remove_block(pos, false);
    }
}

fn decaying(state: BlockState) -> bool {
    !state.get(PERSISTENT) && state.get(DISTANCE) == 7
}

pub fn tick(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let updated = update_distance(state, level, pos);
    level.set_block(pos, updated, UPDATE_ALL);
}

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos, neighbour_state: BlockState) -> BlockState {
    if state.get(WATERLOGGED) {
        schedule_water_tick(level, pos);
    }
    let distance_from_neighbor = distance_at(neighbour_state) + 1;
    if distance_from_neighbor != 1 || state.get(DISTANCE) != distance_from_neighbor {
        level.schedule_tick(pos, state.block(), 1);
    }
    state
}

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> BlockState {
    let replaced_fluid = level.fluid_state(ctx.clicked_pos());
    let state = block.default_state().set(PERSISTENT, true).set(WATERLOGGED, replaced_fluid.fluid == Fluid::Water);
    update_distance(state, level, ctx.clicked_pos())
}

pub struct LeavesBlock;

impl BlockBehaviour for LeavesBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(get_state_for_placement(block, ctx, level))
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        update_shape(state, level, pos, neighbour_state)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        tick(state, level, pos);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        random_tick(state, level, pos);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::{DISTANCE, PERSISTENT};

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    const LEAVES: BlockPos = BlockPos::new(0, -55, 0);

    #[test]
    fn a_log_next_to_leaves_updates_their_distance_a_tick_later() {
        let mut level = testing::level();
        level.set_block(LEAVES, blocks::OAK_LEAVES.default_state(), UPDATE_ALL);
        assert!(!level.has_scheduled_tick(LEAVES, blocks::OAK_LEAVES));
        level.set_block(LEAVES.east(), blocks::OAK_LOG.default_state(), UPDATE_ALL);
        assert!(level.has_scheduled_tick(LEAVES, blocks::OAK_LEAVES));
        let state = level.block_state(LEAVES);
        block::of(state).tick(state, &mut level, LEAVES);
        assert_eq!(level.block_state(LEAVES).get(DISTANCE), 1);
    }

    #[test]
    fn only_natural_leaves_far_from_logs_decay() {
        let mut level = testing::level();
        let natural = blocks::OAK_LEAVES.default_state();
        let placed = natural.set(PERSISTENT, true);
        level.set_block(LEAVES, natural, UPDATE_ALL);
        level.set_block(LEAVES.east(), placed, UPDATE_ALL);
        for pos in [LEAVES, LEAVES.east()] {
            let state = level.block_state(pos);
            block::of(state).random_tick(state, &mut level, pos);
        }
        assert!(level.block_state(LEAVES).is_air());
        assert_eq!(level.block_state(LEAVES.east()), placed);
    }
}
