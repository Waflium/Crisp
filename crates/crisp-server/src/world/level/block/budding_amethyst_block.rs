use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{FACING, WATERLOGGED};
use crisp_data::{BlockState, Fluid, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct BuddingAmethystBlock;

pub fn can_cluster_grow_at_state(state: BlockState) -> bool {
    state.is_air() || state.is_block(blocks::WATER) && state.fluid_state().amount == 8
}

impl BlockBehaviour for BuddingAmethystBlock {
    fn on_projectile_hit(&self, level: &mut ServerLevel, _state: BlockState, hit_result: &super::BlockHitResult, _projectile: crate::world::entity::EntityId) {
        super::amethyst_block::on_projectile_hit(level, hit_result);
    }

    fn random_tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.random.next_int_bounded(5) != 0 {
            return;
        }
        let grow_direction = Direction::VALUES[level.random.next_int_bounded(Direction::VALUES.len() as i32) as usize];
        let grow_pos = pos.relative(grow_direction);
        let relative_state = level.block_state(grow_pos);
        let next_stage = if can_cluster_grow_at_state(relative_state) {
            Some(blocks::SMALL_AMETHYST_BUD)
        } else if relative_state.is_block(blocks::SMALL_AMETHYST_BUD) && relative_state.get(FACING) == grow_direction {
            Some(blocks::MEDIUM_AMETHYST_BUD)
        } else if relative_state.is_block(blocks::MEDIUM_AMETHYST_BUD) && relative_state.get(FACING) == grow_direction {
            Some(blocks::LARGE_AMETHYST_BUD)
        } else if relative_state.is_block(blocks::LARGE_AMETHYST_BUD) && relative_state.get(FACING) == grow_direction {
            Some(blocks::AMETHYST_CLUSTER)
        } else {
            None
        };
        if let Some(next_stage) = next_stage {
            let target_state = next_stage.default_state().set(FACING, grow_direction).set(WATERLOGGED, relative_state.fluid_state().fluid == Fluid::Water);
            level.set_block(grow_pos, target_state, UPDATE_ALL);
        }
    }
}
