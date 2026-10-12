use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::item::dye_color::DyeColor;

pub struct BeaconBlock;

impl BeaconBlock {
    pub fn get_color(&self) -> DyeColor {
        DyeColor::VALUES[0]
    }
}

impl BlockBehaviour for BeaconBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if let Some(BlockEntityKind::Beacon(_)) = level.get_block_entity(pos).map(|entity| &entity.kind) {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
        }
        InteractionResult::SUCCESS
    }
}
