use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, SupportType, blocks};

use super::BlockBehaviour;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::block;

pub struct LadderBlock;

impl BlockBehaviour for LadderBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        if !ctx.replacing_clicked_on_block() {
            let state = level.block_state(ctx.clicked_pos().relative(ctx.clicked_face().opposite()));
            if state.is_block(block) && state.get(HORIZONTAL_FACING) == ctx.clicked_face() {
                return None;
            }
        }
        let mut state = block.default_state();
        let pos = ctx.clicked_pos();
        let water = is_water_source(level, pos);
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() {
                state = state.set(HORIZONTAL_FACING, direction.opposite());
                if block::of(state).can_survive(state, level, pos) {
                    return Some(state.set(WATERLOGGED, water));
                }
            }
        }
        None
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let direction = state.get(HORIZONTAL_FACING);
        let support = pos.relative(direction.opposite());
        level.block_state(support).is_face_sturdy(direction, SupportType::Full)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if direction.opposite() == state.get(HORIZONTAL_FACING) && !block::of(state).can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        schedule_water_tick(state, level, pos);
        state
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::HORIZONTAL_FACING;
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn hangs_on_sturdy_faces_and_pops_off() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 0.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::LADDER, 64));
        let wall = BlockPos::new(0, -60, 0);
        level.set_block(wall, blocks::STONE.default_state(), UPDATE_ALL);
        click(&mut level, player, wall, Direction::South, Vec3::new(0.5, -59.5, 1.0));
        let pos = wall.south();
        assert_eq!(level.block_state(pos), blocks::LADDER.default_state().set(HORIZONTAL_FACING, Direction::South));
        click(&mut level, player, BlockPos::new(4, -61, 0), Direction::Up, Vec3::new(4.5, -60.0, 0.5));
        assert!(level.block_state(BlockPos::new(4, -60, 0)).is_air());

        level.set_block(wall, blocks::AIR.default_state(), UPDATE_ALL);
        assert!(level.block_state(pos).is_air());
    }
}
