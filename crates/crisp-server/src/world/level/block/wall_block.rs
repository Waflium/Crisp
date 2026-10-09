use std::sync::LazyLock;

use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::properties::{EAST_WALL, EnumProperty, NORTH_WALL, SOUTH_WALL, UP, WATERLOGGED, WEST_WALL, WallSide};
use crisp_data::{Block, BlockState, SupportType, block_tags};

use super::fence_gate_block::connects_to_direction;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::{BlockBehaviour, is_exception_for_connection};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub struct WallBlock;

fn shape(min_x: f64, min_z: f64, max_x: f64, max_z: f64) -> VoxelShape {
    VoxelShape::from_boxes(&[AABB::new(min_x / 16.0, 0.0, min_z / 16.0, max_x / 16.0, 1.0, max_z / 16.0)])
}

static TEST_SHAPE_POST: LazyLock<VoxelShape> = LazyLock::new(|| shape(7.0, 7.0, 9.0, 9.0));

static TEST_SHAPES_WALL: LazyLock<[VoxelShape; 4]> =
    LazyLock::new(|| [shape(7.0, 0.0, 9.0, 9.0), shape(7.0, 7.0, 16.0, 9.0), shape(7.0, 7.0, 9.0, 16.0), shape(0.0, 7.0, 9.0, 9.0)]);

fn is_connected(state: BlockState, property: EnumProperty<WallSide>) -> bool {
    state.get(property) != WallSide::None
}

fn is_covered(above_shape: &VoxelShape, test_shape: &VoxelShape) -> bool {
    !join_is_not_empty(test_shape, above_shape, BooleanOp::OnlyFirst)
}

impl WallBlock {
    fn connects_to(state: BlockState, face_solid: bool, direction: Direction) -> bool {
        let block = state.block();
        let connected_fence_gate = block.is_instance_of("FenceGateBlock") && connects_to_direction(state, direction);
        state.is(&block_tags::WALLS) || !is_exception_for_connection(state) && face_solid || block.is_instance_of("IronBarsBlock") || connected_fence_gate
    }

    fn connects_to_neighbour(neighbour: BlockState, opposite: Direction) -> bool {
        Self::connects_to(neighbour, neighbour.is_face_sturdy(opposite, SupportType::Full), opposite)
    }

    fn top_update(state: BlockState, top_neighbour: BlockState) -> BlockState {
        let north = is_connected(state, NORTH_WALL);
        let east = is_connected(state, EAST_WALL);
        let south = is_connected(state, SOUTH_WALL);
        let west = is_connected(state, WEST_WALL);
        Self::update_wall_shape(state, top_neighbour, north, east, south, west)
    }

    fn side_update(level: &mut ServerLevel, pos: BlockPos, state: BlockState, neighbour: BlockState, direction: Direction) -> BlockState {
        let opposite = direction.opposite();
        let north = if direction == Direction::North { Self::connects_to_neighbour(neighbour, opposite) } else { is_connected(state, NORTH_WALL) };
        let east = if direction == Direction::East { Self::connects_to_neighbour(neighbour, opposite) } else { is_connected(state, EAST_WALL) };
        let south = if direction == Direction::South { Self::connects_to_neighbour(neighbour, opposite) } else { is_connected(state, SOUTH_WALL) };
        let west = if direction == Direction::West { Self::connects_to_neighbour(neighbour, opposite) } else { is_connected(state, WEST_WALL) };
        let above = pos.above();
        let above_state = level.block_state(above);
        Self::update_wall_shape(state, above_state, north, east, south, west)
    }

    fn update_wall_shape(state: BlockState, top_neighbour: BlockState, north: bool, east: bool, south: bool, west: bool) -> BlockState {
        let above_shape = VoxelShape::from_boxes(top_neighbour.collision_shape()).face_shape(Direction::Down);
        let sides_updated = Self::update_sides(state, north, east, south, west, &above_shape);
        sides_updated.set(UP, Self::should_raise_post(sides_updated, top_neighbour, &above_shape))
    }

    fn should_raise_post(state: BlockState, top_neighbour: BlockState, above_shape: &VoxelShape) -> bool {
        let top_neighbour_has_post = top_neighbour.block().is_instance_of("WallBlock") && top_neighbour.get(UP);
        if top_neighbour_has_post {
            return true;
        }
        let north_wall = state.get(NORTH_WALL);
        let south_wall = state.get(SOUTH_WALL);
        let east_wall = state.get(EAST_WALL);
        let west_wall = state.get(WEST_WALL);
        let south_none = south_wall == WallSide::None;
        let west_none = west_wall == WallSide::None;
        let east_none = east_wall == WallSide::None;
        let north_none = north_wall == WallSide::None;
        let has_corner = north_none && south_none && west_none && east_none || north_none != south_none || west_none != east_none;
        if has_corner {
            return true;
        }
        let has_high_wall = north_wall == WallSide::Tall && south_wall == WallSide::Tall || east_wall == WallSide::Tall && west_wall == WallSide::Tall;
        if has_high_wall { false } else { top_neighbour.is(&block_tags::WALL_POST_OVERRIDE) || is_covered(above_shape, &TEST_SHAPE_POST) }
    }

    fn update_sides(state: BlockState, north: bool, east: bool, south: bool, west: bool, above_shape: &VoxelShape) -> BlockState {
        let tests = &*TEST_SHAPES_WALL;
        state
            .set(NORTH_WALL, Self::make_wall_state(north, above_shape, &tests[0]))
            .set(EAST_WALL, Self::make_wall_state(east, above_shape, &tests[1]))
            .set(SOUTH_WALL, Self::make_wall_state(south, above_shape, &tests[2]))
            .set(WEST_WALL, Self::make_wall_state(west, above_shape, &tests[3]))
    }

    fn make_wall_state(connects_to_side: bool, above_shape: &VoxelShape, test_shape: &VoxelShape) -> WallSide {
        if !connects_to_side {
            WallSide::None
        } else if is_covered(above_shape, test_shape) {
            WallSide::Tall
        } else {
            WallSide::Low
        }
    }
}

impl BlockBehaviour for WallBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let water = is_water_source(level, pos);
        let top_pos = pos.above();
        let north_state = level.block_state(pos.north());
        let east_state = level.block_state(pos.east());
        let south_state = level.block_state(pos.south());
        let west_state = level.block_state(pos.west());
        let top_state = level.block_state(top_pos);
        let north = Self::connects_to_neighbour(north_state, Direction::South);
        let east = Self::connects_to_neighbour(east_state, Direction::West);
        let south = Self::connects_to_neighbour(south_state, Direction::North);
        let west = Self::connects_to_neighbour(west_state, Direction::East);
        let state = block.default_state().set(WATERLOGGED, water);
        Some(Self::update_wall_shape(state, top_state, north, east, south, west))
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
        schedule_water_tick(state, level, pos);
        match direction {
            Direction::Down => state,
            Direction::Up => Self::top_update(state, neighbour_state),
            _ => Self::side_update(level, pos, state, neighbour_state, direction),
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::{EAST_WALL, NORTH_WALL, SLAB_TYPE, SOUTH_WALL, SlabType, UP, WEST_WALL, WallSide};

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn low_tall_and_post() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        let wall = blocks::COBBLESTONE_WALL.default_state();
        level.set_block(pos, wall, UPDATE_ALL);
        level.set_block(pos.north(), wall, UPDATE_ALL);
        level.set_block(pos.south(), wall, UPDATE_ALL);
        let state = level.block_state(pos);
        assert_eq!(state.get(NORTH_WALL), WallSide::Low);
        assert_eq!(state.get(SOUTH_WALL), WallSide::Low);
        assert_eq!(state.get(EAST_WALL), WallSide::None);
        assert!(!state.get(UP));
        level.set_block(pos.above(), blocks::TORCH.default_state(), UPDATE_ALL);
        assert!(level.block_state(pos).get(UP));

        level.set_block(pos.above(), blocks::STONE.default_state(), UPDATE_ALL);
        let state = level.block_state(pos);
        assert_eq!(state.get(NORTH_WALL), WallSide::Tall);
        assert_eq!(state.get(SOUTH_WALL), WallSide::Tall);
        assert!(!state.get(UP));

        level.set_block(pos.above(), blocks::STONE_SLAB.default_state().set(SLAB_TYPE, SlabType::Top), UPDATE_ALL);
        assert_eq!(level.block_state(pos).get(NORTH_WALL), WallSide::Low);
        level.set_block(pos.above(), blocks::STONE_SLAB.default_state(), UPDATE_ALL);
        assert_eq!(level.block_state(pos).get(NORTH_WALL), WallSide::Tall);

        level.set_block(pos.south(), blocks::AIR.default_state(), UPDATE_ALL);
        let state = level.block_state(pos);
        assert!(state.get(UP));
        assert_eq!(state.get(WEST_WALL), WallSide::None);
    }
}
