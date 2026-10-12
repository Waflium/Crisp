use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::dye_color::DyeColor;

pub struct BeaconBlock;

impl BeaconBlock {
    pub fn get_color(&self) -> DyeColor {
        DyeColor::VALUES[0]
    }
}

impl BlockBehaviour for BeaconBlock {
    fn use_without_item(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        InteractionResult::SUCCESS
    }
}
