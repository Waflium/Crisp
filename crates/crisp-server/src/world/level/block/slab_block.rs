use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{SLAB_TYPE, SlabType, WATERLOGGED};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub struct SlabBlock;

impl BlockBehaviour for SlabBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let replaced = level.block_state(pos);
        if replaced.is_block(block) {
            return Some(replaced.set(SLAB_TYPE, SlabType::Double).set(WATERLOGGED, false));
        }
        let result = block.default_state().set(SLAB_TYPE, SlabType::Bottom).set(WATERLOGGED, is_water_source(level, pos));
        let clicked_face = ctx.clicked_face();
        if clicked_face != Direction::Down && (clicked_face == Direction::Up || !(ctx.click_location().y - pos.y as f64 > 0.5)) {
            Some(result)
        } else {
            Some(result.set(SLAB_TYPE, SlabType::Top))
        }
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        let slab_type = state.get(SLAB_TYPE);
        if slab_type == SlabType::Double || ctx.item.item != state.block().item() {
            return false;
        }
        if !ctx.replacing_clicked_on_block() {
            return true;
        }
        let above = ctx.click_location().y - ctx.clicked_pos().y as f64 > 0.5;
        let clicked_face = ctx.clicked_face();
        if slab_type == SlabType::Bottom {
            clicked_face == Direction::Up || above && clicked_face.axis().is_horizontal()
        } else {
            clicked_face == Direction::Down || !above && clicked_face.axis().is_horizontal()
        }
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{SLAB_TYPE, SlabType};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;

    #[test]
    fn halves_and_double() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 4.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::OAK_SLAB, 64));
        let grass = BlockPos::new(0, -61, 0);
        let pos = grass.above();
        click(&mut level, player, grass, Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        assert_eq!(level.block_state(pos).get(SLAB_TYPE), SlabType::Bottom);
        click(&mut level, player, pos, Direction::Up, Vec3::new(0.5, -59.5, 0.5));
        assert_eq!(level.block_state(pos).get(SLAB_TYPE), SlabType::Double);

        let side = BlockPos::new(1, -60, 0);
        click(&mut level, player, pos, Direction::East, Vec3::new(1.0, -59.2, 0.5));
        assert_eq!(level.block_state(side), blocks::OAK_SLAB.default_state().set(SLAB_TYPE, SlabType::Top));
        click(&mut level, player, side, Direction::East, Vec3::new(2.0, -59.7, 0.5));
        assert_eq!(level.block_state(side).get(SLAB_TYPE), SlabType::Double);
        assert!(level.block_state(side.east()).is_air());
    }
}
