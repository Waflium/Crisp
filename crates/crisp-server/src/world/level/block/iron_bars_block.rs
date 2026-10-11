use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{EAST, NORTH, SOUTH, WATERLOGGED, WEST};
use crisp_data::{Block, BlockState, SupportType, block_tags};

use super::cross_collision_block::{self, property_by_direction};
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::{BlockBehaviour, is_exception_for_connection};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct IronBarsBlock;

fn attaches_to(state: BlockState, face_solid: bool) -> bool {
    !is_exception_for_connection(state) && face_solid || state.block().is_instance_of("IronBarsBlock") || state.is(&block_tags::WALLS)
}

impl BlockBehaviour for IronBarsBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        cross_collision_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        cross_collision_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let water = is_water_source(level, pos);
        let (north, south, west, east) = (pos.north(), pos.south(), pos.west(), pos.east());
        let north_state = level.block_state(north);
        let south_state = level.block_state(south);
        let west_state = level.block_state(west);
        let east_state = level.block_state(east);
        let sturdy = |state: BlockState, face: Direction| state.is_face_sturdy(face, SupportType::Full);
        Some(
            block
                .default_state()
                .set(NORTH, attaches_to(north_state, sturdy(north_state, Direction::South)))
                .set(SOUTH, attaches_to(south_state, sturdy(south_state, Direction::North)))
                .set(WEST, attaches_to(west_state, sturdy(west_state, Direction::East)))
                .set(EAST, attaches_to(east_state, sturdy(east_state, Direction::West)))
                .set(WATERLOGGED, water),
        )
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        schedule_water_tick(state, level, pos);
        if !direction.axis().is_horizontal() {
            return state;
        }
        let face_solid = neighbour_state.is_face_sturdy(direction.opposite(), SupportType::Full);
        state.set(property_by_direction(direction), attaches_to(neighbour_state, face_solid))
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::{EAST, NORTH, SOUTH, WEST};

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn panes_attach_to_bars_walls_and_solid_faces() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::WHITE_STAINED_GLASS_PANE.default_state(), UPDATE_ALL);
        level.set_block(pos.north(), blocks::IRON_BARS.default_state(), UPDATE_ALL);
        level.set_block(pos.east(), blocks::COBBLESTONE_WALL.default_state(), UPDATE_ALL);
        level.set_block(pos.south(), blocks::OAK_LEAVES.default_state(), UPDATE_ALL);
        level.set_block(pos.west(), blocks::OAK_PLANKS.default_state(), UPDATE_ALL);
        let pane = level.block_state(pos);
        assert!(pane.get(NORTH) && pane.get(EAST) && pane.get(WEST));
        assert!(!pane.get(SOUTH));
        assert!(level.block_state(pos.north()).get(SOUTH));
    }
}
