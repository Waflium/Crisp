use crisp_core::BlockPos;
use crisp_data::blocks;
use crisp_data::properties::HAS_RECORD;

use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc};
use crate::world::level::BlockGetter;

pub fn try_insert_into_jukebox(level: &mut ServerLevel, pos: BlockPos, to_insert: &mut ItemStack, player: EntityId) -> InteractionResult {
    if !to_insert.has(dc::JUKEBOX_PLAYABLE) {
        return InteractionResult::TryWithEmptyHand;
    }
    let state = level.block_state(pos);
    if state.is_block(blocks::JUKEBOX) && !state.get(HAS_RECORD) {
        let inserted = to_insert.copy_with_count(1);
        to_insert.consume(1, level, Some(player));
        if level.jukebox(pos).is_some() {
            level.jukebox_set_the_item(pos, inserted);
        }
        level.award_stat(player, crate::stats::stats::PLAY_RECORD, 1);
        InteractionResult::SUCCESS
    } else {
        InteractionResult::TryWithEmptyHand
    }
}
