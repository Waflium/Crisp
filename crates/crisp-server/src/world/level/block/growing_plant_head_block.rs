use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AGE_25;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::growing_plant_block::{self, GrowingPlant};
use super::vegetation_block::{fields, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

#[allow(unused_variables)]
pub trait HeadParts: Send + Sync {
    fn body_block(&self) -> Block;

    fn can_attach_to(&self, state: BlockState) -> bool {
        true
    }

    fn can_grow_into(&self, state: BlockState) -> bool;

    fn blocks_to_grow_when_bonemealed(&self, level: &mut ServerLevel) -> i32;

    fn grow_into_state(&self, grow_from_state: BlockState, level: &mut ServerLevel) -> BlockState {
        grow_from_state.cycle(AGE_25)
    }

    fn update_body_after_converted_from_head(&self, head_state: BlockState, body_state: BlockState) -> BlockState {
        body_state
    }

    fn may_place(&self, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> bool {
        true
    }
}

pub struct GrowingPlantHeadBlock<P: HeadParts> {
    pub plant: GrowingPlant,
    pub grow_per_tick_probability: f64,
    pub parts: P,
}

impl<P: HeadParts> GrowingPlantHeadBlock<P> {
    pub fn new(block: Block, parts: P) -> GrowingPlantHeadBlock<P> {
        let grow_per_tick_probability = fields(block)["growPerTickProbability"].as_f64().unwrap_or(0.0);
        GrowingPlantHeadBlock { plant: GrowingPlant::new(block), grow_per_tick_probability, parts }
    }

    fn growth_direction(&self) -> Direction {
        self.plant.growth_direction
    }
}

impl<P: HeadParts> BlockBehaviour for GrowingPlantHeadBlock<P> {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        if !self.parts.may_place(ctx, level) {
            return None;
        }
        let body = self.parts.body_block();
        Some(growing_plant_block::get_state_for_placement(&self.plant, block, body, ctx, level, |level| {
            growing_plant_block::head_state_for_placement(block, level)
        }))
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        growing_plant_block::can_survive(&self.plant, state.block(), self.parts.body_block(), |s| self.parts.can_attach_to(s), level, pos)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        growing_plant_block::tick(state, level, pos);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(AGE_25) < 25 && level.random.next_double() < self.grow_per_tick_probability {
            let growth_pos = pos.relative(self.growth_direction());
            if self.parts.can_grow_into(level.block_state(growth_pos)) {
                let grown = self.parts.grow_into_state(state, level);
                level.set_block(growth_pos, grown, UPDATE_ALL);
            }
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        let head = state.block();
        let body = self.parts.body_block();
        if direction == self.growth_direction().opposite() {
            if !self.can_survive(state, level, pos) {
                level.schedule_tick(pos, head, 1);
            } else {
                let neighbor_in_growth_direction = level.block_state(pos.relative(self.growth_direction()));
                if neighbor_in_growth_direction.is_block(head) || neighbor_in_growth_direction.is_block(body) {
                    return self.parts.update_body_after_converted_from_head(state, body.default_state());
                }
            }
        }
        if direction != self.growth_direction() || !neighbour_state.is_block(head) && !neighbour_state.is_block(body) {
            if self.plant.schedule_fluid_ticks {
                schedule_water_tick(level, pos);
            }
            state
        } else {
            self.parts.update_body_after_converted_from_head(state, body.default_state())
        }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        let growth_pos = pos.relative(self.growth_direction());
        self.parts.can_grow_into(level.block_state(growth_pos)) && !level.chunks.is_outside_build_height(growth_pos.y)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let mut forward_pos = pos.relative(self.growth_direction());
        let mut next_age = (state.get(AGE_25) + 1).min(25);
        let blocks_to_grow = self.parts.blocks_to_grow_when_bonemealed(level);
        let mut i = 0;
        while i < blocks_to_grow && self.parts.can_grow_into(level.block_state(forward_pos)) && !level.chunks.is_outside_build_height(forward_pos.y) {
            level.set_block(forward_pos, state.set(AGE_25, next_age), UPDATE_ALL);
            forward_pos = forward_pos.relative(self.growth_direction());
            next_age = (next_age + 1).min(25);
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_core::random::LegacyRandomSource;
    use crisp_data::blocks;
    use crisp_data::properties::AGE_25;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn a_head_becomes_body_when_the_plant_grows_past_it() {
        let mut level = testing::level();
        let root = BlockPos::new(0, -50, 0);
        level.set_block(root, blocks::NETHERRACK.default_state(), UPDATE_ALL);
        let head = blocks::WEEPING_VINES.default_state().set(AGE_25, 3);
        level.set_block(root.below(), head, UPDATE_ALL);
        level.random = LegacyRandomSource::new(7);
        let mut grew = false;
        for _ in 0..200 {
            let state = level.block_state(root.below());
            block::of(state).random_tick(state, &mut level, root.below());
            if level.block_state(root.below()).is_block(blocks::WEEPING_VINES_PLANT) {
                grew = true;
                break;
            }
        }
        assert!(grew);
        assert_eq!(level.block_state(root.below().below()), head.set(AGE_25, 4));
    }

    #[test]
    fn bone_meal_grows_cave_vine_berries() {
        let mut level = testing::level();
        let root = BlockPos::new(0, -50, 0);
        level.set_block(root, blocks::NETHERRACK.default_state(), UPDATE_ALL);
        level.set_block(root.below(), blocks::CAVE_VINES_PLANT.default_state(), UPDATE_ALL);
        level.set_block(root.below().below(), blocks::CAVE_VINES.default_state(), UPDATE_ALL);
        let body = level.block_state(root.below());
        let behaviour = block::of(body);
        assert!(behaviour.is_valid_bonemeal_target(&mut level, root.below(), body));
        behaviour.perform_bonemeal(&mut level, root.below(), body);
        assert!(level.block_state(root.below()).get(crisp_data::properties::BERRIES));
    }

    #[test]
    fn bone_meal_on_the_body_grows_the_head() {
        let mut level = testing::level();
        let ground = BlockPos::new(0, -61, 0);
        level.set_block(ground.above(), blocks::TWISTING_VINES_PLANT.default_state(), UPDATE_ALL);
        level.set_block(ground.above().above(), blocks::TWISTING_VINES.default_state(), UPDATE_ALL);
        level.random = LegacyRandomSource::new(1);
        let body = level.block_state(ground.above());
        assert!(block::of(body).is_valid_bonemeal_target(&mut level, ground.above(), body));
        block::of(body).perform_bonemeal(&mut level, ground.above(), body);
        assert!(level.block_state(ground.above().above()).is_block(blocks::TWISTING_VINES_PLANT));
        let grown = level.block_state(ground.above().above().above());
        assert!(grown.is_block(blocks::TWISTING_VINES) || grown.is_block(blocks::TWISTING_VINES_PLANT));
    }
}
