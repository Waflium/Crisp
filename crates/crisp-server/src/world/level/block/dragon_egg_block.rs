use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use super::{BlockBehaviour, BlockHitResult, falling_block};
use crate::server::level::ServerLevel;
use crate::server::level::events;
use crate::util::block_util;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::level::update_flags::UPDATE_CLIENTS;

const DELAY_AFTER_PLACE: i32 = 5;

pub struct DragonEggBlock;

fn teleport(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    for _ in 0..1000 {
        let random = &mut level.random;
        let test_pos = pos.offset(
            random.next_int_bounded(16) - random.next_int_bounded(16),
            random.next_int_bounded(8) - random.next_int_bounded(8),
            random.next_int_bounded(16) - random.next_int_bounded(16),
        );
        if level.block_state(test_pos).is_air()
            && !level.block_state(test_pos.below()).is_air()
            && level.world_border.is_within_bounds_pos(test_pos)
            && level.is_inside_build_height(test_pos.y)
        {
            let packed_diff = block_util::pack_difference_in_position(pos, test_pos, 16, 8, 16);
            level.level_event(None, events::PARTICLES_DRAGON_EGG_TELEPORT, pos, packed_diff);
            level.set_block(test_pos, state, UPDATE_CLIENTS);
            level.remove_block(pos, false);
            return;
        }
    }
}

impl BlockBehaviour for DragonEggBlock {
    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        teleport(state, level, pos);
        InteractionResult::SUCCESS
    }

    fn attack(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId) {
        teleport(state, level, pos);
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        falling_block::on_place(level, pos, state.block(), DELAY_AFTER_PLACE);
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
        falling_block::update_shape(state, level, pos, DELAY_AFTER_PLACE)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        falling_block::tick(state, level, pos, |_| {});
    }
}
