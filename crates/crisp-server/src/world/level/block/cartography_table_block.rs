use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;

pub struct CartographyTableBlock;

impl BlockBehaviour for CartographyTableBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        level.open_menu(player, MenuProvider::CartographyTable(pos));
        InteractionResult::SUCCESS
    }
}
