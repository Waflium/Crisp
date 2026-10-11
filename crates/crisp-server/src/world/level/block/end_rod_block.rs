use crisp_data::properties::FACING;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rod_block;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub struct EndRodBlock;

impl BlockBehaviour for EndRodBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        rod_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        rod_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let clicked_face = ctx.clicked_face();
        let state = level.block_state(ctx.clicked_pos().relative(clicked_face.opposite()));
        if state.is_block(block) && state.get(FACING) == clicked_face {
            Some(block.default_state().set(FACING, clicked_face.opposite()))
        } else {
            Some(block.default_state().set(FACING, clicked_face))
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::items;
    use crisp_data::properties::FACING;

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;

    #[test]
    fn rods_placed_end_to_end_face_each_other() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::END_ROD, 64));
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        assert_eq!(level.block_state(pos).get(FACING), Direction::Up);
        click(&mut level, player, pos, Direction::Up, Vec3::new(0.5, -59.0, 0.5));
        assert_eq!(level.block_state(pos.above()).get(FACING), Direction::Down);
    }
}
