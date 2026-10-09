use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntityKind;

pub struct AbstractFurnaceBlock;

impl BlockBehaviour for AbstractFurnaceBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Furnace(_))) {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
        }
        InteractionResult::SUCCESS
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.get_redstone_signal_from_block_entity(pos)
    }
}
