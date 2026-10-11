use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AGE_15;
use crisp_data::{BlockState, block_tags, fluid_tags};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_LIMIT, UPDATE_NONE};

pub struct SugarCaneBlock;

impl BlockBehaviour for SugarCaneBlock {
    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.can_survive(state, level, pos) {
            level.destroy_block(pos, true, None, UPDATE_LIMIT);
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !level.block_state(pos.above()).is_air() {
            return;
        }
        let mut height = 1;
        while level.block_state(pos.offset(0, -height, 0)).is_block(state.block()) {
            height += 1;
        }
        if height < 3 {
            let age = state.get(AGE_15);
            if age == 15 {
                level.set_block(pos.above(), state.block().default_state(), UPDATE_ALL);
                level.set_block(pos, state.set(AGE_15, 0), UPDATE_NONE);
            } else {
                level.set_block(pos, state.set(AGE_15, age + 1), UPDATE_NONE);
            }
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, state.block(), 1);
        }
        state
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let state_below = level.block_state(pos.below());
        if state_below.is_block(state.block()) {
            return true;
        }
        if state_below.is(&block_tags::SUPPORTS_SUGAR_CANE) {
            let below = pos.below();
            for direction in Direction::PLANE_HORIZONTAL {
                let block_state = level.block_state(below.relative(direction));
                let fluid_state = level.fluid_state(below.relative(direction));
                if fluid_state.is(&fluid_tags::SUPPORTS_SUGAR_CANE_ADJACENTLY) || block_state.is(&block_tags::SUPPORTS_SUGAR_CANE_ADJACENTLY) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::AGE_15;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    const CANE: BlockPos = BlockPos::new(0, -60, 0);

    #[test]
    fn sugar_cane_needs_water_next_to_its_ground() {
        let mut level = testing::level();
        let cane = blocks::SUGAR_CANE.default_state();
        assert!(!block::of(cane).can_survive(cane, &mut level, CANE));
        level.set_block(CANE.below().east(), blocks::WATER.default_state(), UPDATE_ALL);
        assert!(block::of(cane).can_survive(cane, &mut level, CANE));
        level.set_block(CANE, cane, UPDATE_ALL);

        level.set_block(CANE.below().east(), blocks::DIRT.default_state(), UPDATE_ALL);
        assert!(!level.has_scheduled_tick(CANE, blocks::SUGAR_CANE));
        level.set_block(CANE.above(), blocks::STONE.default_state(), UPDATE_ALL);
        assert!(level.has_scheduled_tick(CANE, blocks::SUGAR_CANE));
        block::of(cane).tick(cane, &mut level, CANE);
        assert!(level.block_state(CANE).is_air());
    }

    #[test]
    fn sugar_cane_grows_every_sixteen_random_ticks_up_to_three_blocks() {
        let mut level = testing::level();
        level.set_block(CANE.below().east(), blocks::WATER.default_state(), UPDATE_ALL);
        level.set_block(CANE, blocks::SUGAR_CANE.default_state(), UPDATE_ALL);
        for _ in 0..64 {
            for y in 0..4 {
                let pos = CANE.offset(0, y, 0);
                let state = level.block_state(pos);
                if state.is_block(blocks::SUGAR_CANE) {
                    block::of(state).random_tick(state, &mut level, pos);
                }
            }
        }
        assert!(level.block_state(CANE.offset(0, 2, 0)).is_block(blocks::SUGAR_CANE));
        assert!(level.block_state(CANE.offset(0, 3, 0)).is_air());
        assert_eq!(level.block_state(CANE).get(AGE_15), 0);
    }
}
