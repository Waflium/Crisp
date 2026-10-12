use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AGE_5;
use crisp_data::{Block, BlockState, block_tags};

use super::BlockBehaviour;
use super::chorus_plant_block::get_state_with_connections;
use super::vegetation_block::{block_field, fields};
use crate::server::level::ServerLevel;
use crate::server::level::events::{SOUND_CHORUS_DEATH, SOUND_CHORUS_GROW};
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_LIMIT};
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub const DEAD_AGE: i32 = 5;

fn all_neighbors_empty(level: &mut dyn LevelReader, pos: BlockPos, ignore: Option<Direction>) -> bool {
    Direction::PLANE_HORIZONTAL.into_iter().all(|direction| Some(direction) == ignore || level.block_state(pos.relative(direction)).is_air())
}

pub struct ChorusFlowerBlock {
    block: Block,
    plant: Block,
}

impl ChorusFlowerBlock {
    pub fn new(block: Block) -> ChorusFlowerBlock {
        ChorusFlowerBlock { block, plant: block_field(&fields(block), "plant") }
    }

    fn place_grown_flower(&self, level: &mut ServerLevel, pos: BlockPos, age: i32) {
        level.set_block(pos, self.block.default_state().set(AGE_5, age), UPDATE_CLIENTS);
        level.level_event(None, SOUND_CHORUS_GROW, pos, 0);
    }

    fn place_dead_flower(&self, level: &mut ServerLevel, pos: BlockPos) {
        level.set_block(pos, self.block.default_state().set(AGE_5, DEAD_AGE), UPDATE_CLIENTS);
        level.level_event(None, SOUND_CHORUS_DEATH, pos, 0);
    }
}

impl BlockBehaviour for ChorusFlowerBlock {
    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.can_survive(state, level, pos) {
            level.destroy_block(pos, true, None, UPDATE_LIMIT);
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let above = pos.above();
        if !(level.block_state(above).is_air() && above.y <= level.chunks.max_y()) {
            return;
        }
        let current_age = state.get(AGE_5);
        if current_age >= DEAD_AGE {
            return;
        }
        let mut grow_upwards = false;
        let mut pillar_on_support_block = false;
        let below_state = level.block_state(pos.below());
        if below_state.is(&block_tags::SUPPORTS_CHORUS_FLOWER) {
            grow_upwards = true;
        } else if below_state.is_block(self.plant) {
            let mut height = 1;
            for _ in 0..4 {
                let test_state = level.block_state(pos.offset(0, -(height + 1), 0));
                if !test_state.is_block(self.plant) {
                    if test_state.is(&block_tags::SUPPORTS_CHORUS_FLOWER) {
                        pillar_on_support_block = true;
                    }
                    break;
                }
                height += 1;
            }
            if height < 2 || height <= level.random.next_int_bounded(if pillar_on_support_block { 5 } else { 4 }) {
                grow_upwards = true;
            }
        } else if below_state.is_air() {
            grow_upwards = true;
        }
        if grow_upwards && all_neighbors_empty(level, above, None) && level.block_state(pos.offset(0, 2, 0)).is_air() {
            let plant = get_state_with_connections(level, pos, self.plant.default_state());
            level.set_block(pos, plant, UPDATE_CLIENTS);
            self.place_grown_flower(level, above, current_age);
        } else if current_age < 4 {
            let mut num_branch_attempts = level.random.next_int_bounded(4);
            if pillar_on_support_block {
                num_branch_attempts += 1;
            }
            let mut created_branch = false;
            for _ in 0..num_branch_attempts {
                let direction = Direction::PLANE_HORIZONTAL[level.random.next_int_bounded(4) as usize];
                let target = pos.relative(direction);
                if level.block_state(target).is_air() && level.block_state(target.below()).is_air() && all_neighbors_empty(level, target, Some(direction.opposite())) {
                    self.place_grown_flower(level, target, current_age + 1);
                    created_branch = true;
                }
            }
            if created_branch {
                let plant = get_state_with_connections(level, pos, self.plant.default_state());
                level.set_block(pos, plant, UPDATE_CLIENTS);
            } else {
                self.place_dead_flower(level, pos);
            }
        } else {
            self.place_dead_flower(level, pos);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction != Direction::Up && !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, self.block, 1);
        }
        state
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let below_state = level.block_state(pos.below());
        if below_state.is_block(self.plant) || below_state.is(&block_tags::SUPPORTS_CHORUS_FLOWER) {
            return true;
        }
        if !below_state.is_air() {
            return false;
        }
        let mut one_neighbor = false;
        for direction in Direction::PLANE_HORIZONTAL {
            let neighbor = level.block_state(pos.relative(direction));
            if neighbor.is_block(self.plant) {
                if one_neighbor {
                    return false;
                }
                one_neighbor = true;
            } else if !neighbor.is_air() {
                return false;
            }
        }
        one_neighbor
    }
}
