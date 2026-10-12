use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, block_tags};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::entity::item::falling_block_entity::FallingBlockEntity;
use crate::world::level::{BlockGetter, LevelAccessor};

pub const DEFAULT_DELAY_AFTER_PLACE: i32 = 2;

pub fn on_place(level: &mut ServerLevel, pos: BlockPos, block: Block, delay_after_place: i32) {
    level.schedule_tick(pos, block, delay_after_place);
}

pub fn update_shape(state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, delay_after_place: i32) -> BlockState {
    level.schedule_tick(pos, state.block(), delay_after_place);
    state
}

pub fn tick(state: BlockState, level: &mut ServerLevel, pos: BlockPos, falling: impl FnOnce(&mut FallingBlockEntity)) {
    if is_free(level.block_state(pos.below())) && pos.y >= level.chunks.min_y() {
        let entity = crate::world::entity::item::falling_block_entity::fall(level, pos, state);
        falling(level.falling_block(entity));
    }
}

pub fn is_free(state: BlockState) -> bool {
    state.is_air() || state.is(&block_tags::FIRE) || state.liquid() || state.can_be_replaced()
}

pub struct FallingBlock;

impl BlockBehaviour for FallingBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        on_place(level, pos, state.block(), DEFAULT_DELAY_AFTER_PLACE);
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
        update_shape(state, level, pos, DEFAULT_DELAY_AFTER_PLACE)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        tick(state, level, pos, |_| {});
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;

    use crate::server::level::testing;
    use crate::world::entity::LevelEntity;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn sand_falls_and_lands_on_the_ground() {
        let mut level = testing::level();
        let player = testing::player(&mut level, crisp_core::Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.handle_player_loaded(player);
        let pos = BlockPos::new(0, -50, 0);
        level.set_block(pos, blocks::SAND.default_state(), UPDATE_ALL);
        for _ in 0..3 {
            level.tick();
        }
        assert!(level.block_state(pos).is_air());
        assert_eq!(level.entities.values().filter(|e| matches!(e, LevelEntity::FallingBlock(_))).count(), 1);
        for _ in 0..40 {
            level.tick();
        }
        assert_eq!(level.block_state(BlockPos::new(0, -60, 0)), blocks::SAND.default_state());
        assert!(!level.entities.values().any(|e| matches!(e, LevelEntity::FallingBlock(_))));
    }
}
