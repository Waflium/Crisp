use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;
use crisp_data::properties::{BooleanProperty, HAS_BOTTLE_0, HAS_BOTTLE_1, HAS_BOTTLE_2};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::level::block::entity::BlockEntityKind;

pub const HAS_BOTTLE: [BooleanProperty; 3] = [HAS_BOTTLE_0, HAS_BOTTLE_1, HAS_BOTTLE_2];

pub struct BrewingStandBlock;

impl BlockBehaviour for BrewingStandBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::BrewingStand(_))) {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
        }
        InteractionResult::SUCCESS
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.get_redstone_signal_from_block_entity(pos)
    }
}
