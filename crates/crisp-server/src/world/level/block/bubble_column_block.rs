use crisp_core::{BlockPos, Direction};
use crisp_data::properties::DRAG;
use crisp_data::{BlockState, Fluid, block_tags, blocks, fluid_tags, items};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::material::{self, tick_delay};

const CHECK_PERIOD: i32 = 5;

pub struct BubbleColumnBlock;

pub fn update_column(level: &mut ServerLevel, occupy_at: BlockPos, below_state: BlockState) {
    let occupy_state = level.block_state(occupy_at);
    update_column_with_state(level, occupy_at, occupy_state, below_state);
}

fn update_column_with_state(level: &mut ServerLevel, occupy_at: BlockPos, occupy_state: BlockState, below_state: BlockState) {
    if !can_occupy(occupy_state) {
        return;
    }
    let column_state = column_state(below_state, occupy_state);
    level.set_block(occupy_at, column_state, 2);
    let mut pos = occupy_at.above();
    while can_occupy(level.block_state(pos)) {
        if !level.set_block(pos, column_state, 2) {
            return;
        }
        pos = pos.above();
    }
}

fn can_occupy(occupy_state: BlockState) -> bool {
    if occupy_state.is_block(blocks::BUBBLE_COLUMN) {
        return true;
    }
    let fluid = occupy_state.fluid_state();
    fluid.is(&fluid_tags::BUBBLE_COLUMN_CAN_OCCUPY) && occupy_state.block().is_instance_of("LiquidBlock") && fluid.is_source() && fluid.amount >= 8
}

fn column_state(below_state: BlockState, occupy_state: BlockState) -> BlockState {
    if below_state.is_block(blocks::BUBBLE_COLUMN) {
        below_state
    } else if below_state.is(&block_tags::ENABLES_BUBBLE_COLUMN_PUSH_UP) {
        blocks::BUBBLE_COLUMN.default_state().set(DRAG, false)
    } else if below_state.is(&block_tags::ENABLES_BUBBLE_COLUMN_DRAG_DOWN) {
        blocks::BUBBLE_COLUMN.default_state().set(DRAG, true)
    } else if occupy_state.is_block(blocks::BUBBLE_COLUMN) {
        blocks::WATER.default_state()
    } else {
        occupy_state
    }
}

impl BlockBehaviour for BubbleColumnBlock {
    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let below = level.block_state(pos.below());
        update_column_with_state(level, pos, state, below);
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        level.schedule_fluid_tick(pos, Fluid::Water, tick_delay(level, Fluid::Water));
        if !self.can_survive(state, level, pos)
            || direction == Direction::Down
            || direction == Direction::Up && !neighbour_state.is_block(blocks::BUBBLE_COLUMN) && can_occupy(neighbour_state)
        {
            level.schedule_tick(pos, blocks::BUBBLE_COLUMN, CHECK_PERIOD);
        }
        state
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let below = level.block_state(pos.below());
        below.is_block(blocks::BUBBLE_COLUMN) || below.is(&block_tags::ENABLES_BUBBLE_COLUMN_PUSH_UP) || below.is(&block_tags::ENABLES_BUBBLE_COLUMN_DRAG_DOWN)
    }

    fn is_bucket_pickup(&self, _state: BlockState) -> bool {
        true
    }

    fn pickup_block(&self, _player: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> Option<ItemStack> {
        level.set_block(pos, blocks::AIR.default_state(), 11);
        Some(ItemStack::new(items::WATER_BUCKET, 1))
    }

    fn pickup_sound(&self) -> Option<i32> {
        material::pickup_sound(Fluid::Water)
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::blocks;
    use crisp_data::properties::DRAG;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn soul_sand_starts_an_upward_column_and_air_ends_it() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(40.5, -60.0, 40.5), 0.0, 0.0);
        let soul_sand = BlockPos::new(0, -61, 0);
        level.set_block(soul_sand, blocks::SOUL_SAND.default_state(), UPDATE_ALL);
        level.set_block(soul_sand.above(), blocks::WATER.default_state(), UPDATE_ALL);
        level.set_block(soul_sand.above().above(), blocks::WATER.default_state(), UPDATE_ALL);
        for _ in 0..19 {
            level.tick();
        }
        assert_eq!(level.block_state(soul_sand.above()), blocks::WATER.default_state());
        level.tick();
        let column = blocks::BUBBLE_COLUMN.default_state().set(DRAG, false);
        assert_eq!(level.block_state(soul_sand.above()), column);
        assert_eq!(level.block_state(soul_sand.above().above()), column);
        level.set_block(soul_sand, blocks::DIRT.default_state(), UPDATE_ALL);
        for _ in 0..5 {
            level.tick();
        }
        assert_eq!(level.block_state(soul_sand.above()), blocks::WATER.default_state());
    }
}
