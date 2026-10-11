use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{HALF, HORIZONTAL_FACING, Half, STAIRS_SHAPE, StairsShape, WATERLOGGED};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct StairBlock;

impl BlockBehaviour for StairBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        let direction = state.get(HORIZONTAL_FACING);
        let shape = state.get(STAIRS_SHAPE);
        match mirror {
            Mirror::LeftRight if direction.axis() == Axis::Z => match shape {
                StairsShape::OuterLeft => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::OuterRight),
                StairsShape::InnerRight => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::InnerLeft),
                StairsShape::InnerLeft => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::InnerRight),
                StairsShape::OuterRight => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::OuterLeft),
                StairsShape::Straight => super::rotate(state, Rotation::Clockwise180),
            },
            Mirror::FrontBack if direction.axis() == Axis::X => match shape {
                StairsShape::Straight => super::rotate(state, Rotation::Clockwise180),
                StairsShape::OuterLeft => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::OuterRight),
                StairsShape::InnerRight => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::InnerRight),
                StairsShape::InnerLeft => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::InnerLeft),
                StairsShape::OuterRight => super::rotate(state, Rotation::Clockwise180).set(STAIRS_SHAPE, StairsShape::OuterLeft),
            },
            _ => state,
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let clicked_face = ctx.clicked_face();
        let pos = ctx.clicked_pos();
        let water = is_water_source(level, pos);
        let half = if clicked_face != Direction::Down && (clicked_face == Direction::Up || !(ctx.click_location().y - pos.y as f64 > 0.5)) {
            Half::Bottom
        } else {
            Half::Top
        };
        let state = block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction()).set(HALF, half).set(WATERLOGGED, water);
        Some(state.set(STAIRS_SHAPE, stairs_shape(state, level, pos)))
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
        if direction.axis().is_horizontal() { state.set(STAIRS_SHAPE, stairs_shape(state, level, pos)) } else { state }
    }
}

fn stairs_shape(state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> StairsShape {
    let facing = state.get(HORIZONTAL_FACING);
    let behind = level.block_state(pos.relative(facing));
    if is_stairs(behind) && state.get(HALF) == behind.get(HALF) {
        let behind_facing = behind.get(HORIZONTAL_FACING);
        if behind_facing.axis() != state.get(HORIZONTAL_FACING).axis() && can_take_shape(state, level, pos, behind_facing.opposite()) {
            return if behind_facing == facing.counter_clockwise() { StairsShape::OuterLeft } else { StairsShape::OuterRight };
        }
    }
    let front = level.block_state(pos.relative(facing.opposite()));
    if is_stairs(front) && state.get(HALF) == front.get(HALF) {
        let front_facing = front.get(HORIZONTAL_FACING);
        if front_facing.axis() != state.get(HORIZONTAL_FACING).axis() && can_take_shape(state, level, pos, front_facing) {
            return if front_facing == facing.counter_clockwise() { StairsShape::InnerLeft } else { StairsShape::InnerRight };
        }
    }
    StairsShape::Straight
}

fn can_take_shape(state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, neighbour: Direction) -> bool {
    let neighbour_state = level.block_state(pos.relative(neighbour));
    !is_stairs(neighbour_state)
        || neighbour_state.get(HORIZONTAL_FACING) != state.get(HORIZONTAL_FACING)
        || neighbour_state.get(HALF) != state.get(HALF)
}

fn is_stairs(state: BlockState) -> bool {
    state.block().is_instance_of("StairBlock")
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{HALF, HORIZONTAL_FACING, Half, STAIRS_SHAPE, StairsShape};
    use crisp_data::{BlockState, blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn stairs(facing: Direction) -> BlockState {
        blocks::OAK_STAIRS.default_state().set(HORIZONTAL_FACING, facing)
    }

    #[test]
    fn placement_faces_the_look_direction() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 4.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::OAK_STAIRS, 64));
        let grass = BlockPos::new(0, -61, 0);
        click(&mut level, player, grass, Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        let placed = level.block_state(grass.above());
        assert_eq!(placed.get(HORIZONTAL_FACING), Direction::North);
        assert_eq!(placed.get(HALF), Half::Bottom);
        click(&mut level, player, grass.above(), Direction::East, Vec3::new(1.0, -59.3, 0.5));
        assert_eq!(level.block_state(BlockPos::new(1, -60, 0)).get(HALF), Half::Top);
    }

    #[test]
    fn corners_from_neighbours() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, stairs(Direction::North), UPDATE_ALL);
        level.set_block(pos.north(), stairs(Direction::East), UPDATE_ALL);
        assert_eq!(level.block_state(pos).get(STAIRS_SHAPE), StairsShape::OuterRight);
        level.set_block(pos.north(), stairs(Direction::West), UPDATE_ALL);
        assert_eq!(level.block_state(pos).get(STAIRS_SHAPE), StairsShape::OuterLeft);
        level.set_block(pos.north(), stairs(Direction::West).set(HALF, Half::Top), UPDATE_ALL);
        assert_eq!(level.block_state(pos).get(STAIRS_SHAPE), StairsShape::Straight);
        level.set_block(pos.north(), blocks::AIR.default_state(), UPDATE_ALL);

        level.set_block(pos.south(), stairs(Direction::West), UPDATE_ALL);
        assert_eq!(level.block_state(pos).get(STAIRS_SHAPE), StairsShape::InnerLeft);
        level.set_block(pos.west(), stairs(Direction::North), UPDATE_ALL);
        assert_eq!(level.block_state(pos).get(STAIRS_SHAPE), StairsShape::Straight);
    }
}
