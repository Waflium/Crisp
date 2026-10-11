use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AGE_15;
use crisp_data::{BlockState, block_tags, blocks, fluid_tags};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_LIMIT, UPDATE_NONE};

pub struct CactusBlock;

impl BlockBehaviour for CactusBlock {
    fn entity_inside(
        &self,
        _state: BlockState,
        level: &mut ServerLevel,
        _pos: BlockPos,
        entity: crate::world::entity::EntityId,
        _effects: &mut crate::world::entity::inside_block::InsideBlockEffects,
        _is_precise: bool,
    ) {
        level.hurt(entity, crate::world::damagesource::DamageSource::new(crisp_data::damage_types::CACTUS), 1.0);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.can_survive(state, level, pos) {
            level.destroy_block(pos, true, None, UPDATE_LIMIT);
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let above = pos.above();
        if !level.block_state(above).is_air() {
            return;
        }
        let block = state.block();
        let mut height = 1;
        let age = state.get(AGE_15);
        while level.block_state(pos.offset(0, -height, 0)).is_block(block) {
            height += 1;
            if height == 3 && age == 15 {
                return;
            }
        }
        if age == 8 && self.can_survive(block.default_state(), level, above) {
            let chance_to_grow_flower = if height >= 3 { 0.25 } else { 0.1 };
            if level.random.next_double() <= chance_to_grow_flower {
                level.set_block(above, blocks::CACTUS_FLOWER.default_state(), UPDATE_ALL);
            }
        } else if age == 15 && height < 3 {
            level.set_block(above, block.default_state(), UPDATE_ALL);
            let above_block = state.set(AGE_15, 0);
            level.set_block(pos, above_block, UPDATE_NONE);
            level.neighbor_changed_with_state(above_block, above, block, false);
        }
        if age < 15 {
            level.set_block(pos, state.set(AGE_15, age + 1), UPDATE_NONE);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, state.block(), 1);
        }
        state
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        for direction in Direction::PLANE_HORIZONTAL {
            let neighbour = level.block_state(pos.relative(direction));
            if neighbour.is_solid() || level.fluid_state(pos.relative(direction)).is(&fluid_tags::LAVA) {
                return false;
            }
        }
        let below = level.block_state(pos.below());
        (below.is_block(state.block()) || below.is(&block_tags::SUPPORTS_CACTUS)) && !level.block_state(pos.above()).liquid()
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn cactus_breaks_a_tick_after_a_solid_block_appears_next_to_it() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos.below(), blocks::SAND.default_state(), UPDATE_ALL);
        let cactus = blocks::CACTUS.default_state();
        assert!(block::of(cactus).can_survive(cactus, &mut level, pos));
        level.set_block(pos, cactus, UPDATE_ALL);
        level.set_block(pos.north(), blocks::STONE.default_state(), UPDATE_ALL);
        assert!(level.has_scheduled_tick(pos, blocks::CACTUS));
        block::of(cactus).tick(cactus, &mut level, pos);
        assert!(level.block_state(pos).is_air());
        assert_eq!(level.entities.len(), 1);
    }
}
