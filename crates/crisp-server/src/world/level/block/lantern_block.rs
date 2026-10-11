use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{HANGING, WATERLOGGED};
use crisp_data::{Block, BlockState, blocks};

use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::{BlockBehaviour, can_support_center};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::block;

pub struct LanternBlock;

fn connected_direction(state: BlockState) -> Direction {
    if state.get(HANGING) { Direction::Down } else { Direction::Up }
}

impl BlockBehaviour for LanternBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let water = is_water_source(level, ctx.clicked_pos());
        for direction in ctx.nearest_looking_directions() {
            if direction.axis() == Axis::Y {
                let state = block.default_state().set(HANGING, direction == Direction::Up);
                if block::of(state).can_survive(state, level, ctx.clicked_pos()) {
                    return Some(state.set(WATERLOGGED, water));
                }
            }
        }
        None
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let direction = connected_direction(state).opposite();
        can_support_center(level, pos.relative(direction), direction.opposite())
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        schedule_water_tick(state, level, pos);
        if connected_direction(state).opposite() == direction && !block::of(state).can_survive(state, level, pos) {
            blocks::AIR.default_state()
        } else {
            state
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::HANGING;
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn stands_or_hangs() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::LANTERN, 64));
        let ceiling = BlockPos::new(0, -57, 0);
        level.set_block(ceiling, blocks::STONE.default_state(), UPDATE_ALL);
        click(&mut level, player, ceiling, Direction::Down, Vec3::new(0.5, -57.0, 0.5));
        assert!(level.block_state(ceiling.below()).get(HANGING));
        click(&mut level, player, BlockPos::new(2, -61, 0), Direction::Up, Vec3::new(2.5, -60.0, 0.5));
        assert!(!level.block_state(BlockPos::new(2, -60, 0)).get(HANGING));

        level.set_block(ceiling, blocks::AIR.default_state(), UPDATE_ALL);
        assert!(level.block_state(ceiling.below()).is_air());
    }
}
