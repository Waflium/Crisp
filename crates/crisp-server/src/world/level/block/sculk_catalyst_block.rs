use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::BlockBehaviour;
use super::drop_experience_block::XpRange;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct SculkCatalystBlock;

impl BlockBehaviour for SculkCatalystBlock {
    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {
        if drop_experience {
            level.try_drop_experience(pos, tool.as_ref(), XpRange::Constant(5));
        }
    }
}
