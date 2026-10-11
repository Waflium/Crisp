use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{EAST, NORTH, SOUTH, WATERLOGGED, WEST};
use crisp_data::{Block, BlockState, SupportType, block_tags};

use super::cross_collision_block::{self, property_by_direction};
use super::fence_gate_block::connects_to_direction;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::{BlockBehaviour, is_exception_for_connection};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct FenceBlock;

impl FenceBlock {
    fn connects_to(own: Block, state: BlockState, face_solid: bool, direction: Direction) -> bool {
        let same_fence = Self::is_same_fence(own, state);
        let gate = state.block().is_instance_of("FenceGateBlock") && connects_to_direction(state, direction);
        !is_exception_for_connection(state) && face_solid || same_fence || gate
    }

    fn is_same_fence(own: Block, state: BlockState) -> bool {
        state.is(&block_tags::FENCES) && state.is(&block_tags::WOODEN_FENCES) == own.default_state().is(&block_tags::WOODEN_FENCES)
    }
}

impl BlockBehaviour for FenceBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        cross_collision_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        cross_collision_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let water = is_water_source(level, pos);
        let (north, east, south, west) = (pos.north(), pos.east(), pos.south(), pos.west());
        let north_state = level.block_state(north);
        let east_state = level.block_state(east);
        let south_state = level.block_state(south);
        let west_state = level.block_state(west);
        let sturdy = |state: BlockState, face: Direction| state.is_face_sturdy(face, SupportType::Full);
        Some(
            block
                .default_state()
                .set(NORTH, Self::connects_to(block, north_state, sturdy(north_state, Direction::South), Direction::South))
                .set(EAST, Self::connects_to(block, east_state, sturdy(east_state, Direction::West), Direction::West))
                .set(SOUTH, Self::connects_to(block, south_state, sturdy(south_state, Direction::North), Direction::North))
                .set(WEST, Self::connects_to(block, west_state, sturdy(west_state, Direction::East), Direction::East))
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
        let opposite = direction.opposite();
        let face_solid = neighbour_state.is_face_sturdy(opposite, SupportType::Full);
        state.set(property_by_direction(direction), Self::connects_to(state.block(), neighbour_state, face_solid, opposite))
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{EAST, NORTH, SOUTH, WEST};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn connections() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos.north(), blocks::STONE.default_state(), UPDATE_ALL);
        level.set_block(pos.east(), blocks::SPRUCE_FENCE.default_state(), UPDATE_ALL);
        level.set_block(pos.south(), blocks::NETHER_BRICK_FENCE.default_state(), UPDATE_ALL);
        level.set_block(pos.west(), blocks::PUMPKIN.default_state(), UPDATE_ALL);
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 4.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::OAK_FENCE, 64));
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        let fence = level.block_state(pos);
        assert!(fence.get(NORTH));
        assert!(fence.get(EAST));
        assert!(!fence.get(SOUTH));
        assert!(!fence.get(WEST));
        assert!(level.block_state(pos.east()).get(WEST));

        let gate = blocks::OAK_FENCE_GATE.default_state();
        level.set_block(pos.west(), gate.set(crisp_data::properties::HORIZONTAL_FACING, Direction::North), UPDATE_ALL);
        assert!(level.block_state(pos).get(WEST));
        level.set_block(pos.west(), gate.set(crisp_data::properties::HORIZONTAL_FACING, Direction::East), UPDATE_ALL);
        assert!(!level.block_state(pos).get(WEST));
        level.set_block(pos.north(), blocks::AIR.default_state(), UPDATE_ALL);
        assert!(!level.block_state(pos).get(NORTH));
    }
}
