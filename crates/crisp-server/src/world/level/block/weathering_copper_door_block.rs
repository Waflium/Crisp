use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf};
use crisp_data::{Block, BlockState};

use super::door_block::DoorBlock;
use super::weathering_copper::change_over_time;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;

pub struct WeatheringCopperDoorBlock {
    door: DoorBlock,
}

impl WeatheringCopperDoorBlock {
    pub fn new(block: Block) -> WeatheringCopperDoorBlock {
        WeatheringCopperDoorBlock { door: DoorBlock::new(block) }
    }
}

impl BlockBehaviour for WeatheringCopperDoorBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        self.door.get_state_for_placement(block, ctx, level)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, placer: Option<EntityId>, item: ItemStack) {
        self.door.set_placed_by(level, pos, state, placer, item);
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
        self.door.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        self.door.player_will_destroy(level, pos, state, player)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hit: &BlockHitResult) -> InteractionResult {
        self.door.use_without_item(state, level, pos, player, hit)
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        self.door.neighbor_changed(state, level, pos, block, moved_by_piston);
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        self.door.can_survive(state, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower {
            change_over_time(state, level, pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf, HORIZONTAL_FACING};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::{self, click};
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn both_halves_age_together() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 90.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::EXPOSED_COPPER_DOOR, 1));
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        level.set_block(BlockPos::new(2, -60, 0), blocks::OXIDIZED_COPPER.default_state(), UPDATE_ALL);
        let lower = level.block_state(pos);
        for _ in 0..1000 {
            block::of(lower).random_tick(lower, &mut level, pos);
            if !level.block_state(pos).is_block(blocks::EXPOSED_COPPER_DOOR) {
                break;
            }
        }
        let aged = level.block_state(pos);
        assert!(aged.is_block(blocks::WEATHERED_COPPER_DOOR));
        assert_eq!(aged.get(HORIZONTAL_FACING), lower.get(HORIZONTAL_FACING));
        assert_eq!(level.block_state(pos.above()), aged.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper));
    }
}
