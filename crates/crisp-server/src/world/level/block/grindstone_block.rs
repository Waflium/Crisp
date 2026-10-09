use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::face_attached_horizontal_directional_block::FaceAttachedHorizontalDirectionalBlock;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;

pub struct GrindstoneBlock;

impl BlockBehaviour for GrindstoneBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        FaceAttachedHorizontalDirectionalBlock.get_state_for_placement(block, ctx, level)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        FaceAttachedHorizontalDirectionalBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn can_survive(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> bool {
        true
    }

    fn use_without_item(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        InteractionResult::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{ATTACH_FACE, AttachFace, HORIZONTAL_FACING};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn placed_on_a_floor_or_against_air() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 0.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::GRINDSTONE, 1));
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        let state = level.block_state(pos);
        assert_eq!(state.get(ATTACH_FACE), AttachFace::Floor);
        assert_eq!(state.get(HORIZONTAL_FACING), Direction::North);

        let wall = BlockPos::new(3, -60, 0);
        level.set_block(wall, blocks::STONE.default_state(), UPDATE_ALL);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::GRINDSTONE, 1));
        click(&mut level, player, wall, Direction::East, Vec3::new(4.0, -59.5, 0.5));
        let side = level.block_state(wall.east());
        assert_eq!(side.get(ATTACH_FACE), AttachFace::Wall);
        assert_eq!(side.get(HORIZONTAL_FACING), Direction::East);
    }
}
