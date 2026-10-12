use crisp_data::blocks;
use crisp_data::properties::EYE;

use super::ItemStack;
use crate::server::level::ServerLevel;
use crate::server::level::events::{END_PORTAL_FRAME_FILL, SOUND_END_PORTAL_SPAWN};
use crate::server::level::game_mode::GameType;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, BlockHitResult, end_portal_frame_block};
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, stack: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let target_state = level.block_state(pos);
    if !target_state.is_block(blocks::END_PORTAL_FRAME) || target_state.get(EYE) {
        return InteractionResult::Pass;
    }
    let new_state = target_state.set(EYE, true);
    block::push_entities_up(target_state, new_state, level, pos);
    level.set_block(pos, new_state, UPDATE_CLIENTS);
    level.update_neighbour_for_output_signal(pos, blocks::END_PORTAL_FRAME);
    let mut item_in_hand = stack;
    item_in_hand.shrink(1);
    if level.player(player).game_mode.game_type != GameType::Creative {
        level.player_mut(player).update_item_in_hand(hand, &item_in_hand);
    }
    level.level_event(None, END_PORTAL_FRAME_FILL, pos, 0);
    if let Some(found) = end_portal_frame_block::get_or_create_portal_shape().find(level, pos) {
        let block_pos = found.get_front_top_left().offset(-3, 0, -3);
        for x in 0..3 {
            for z in 0..3 {
                let portal_block_pos = block_pos.offset(x, 0, z);
                level.destroy_block(portal_block_pos, true, None, 512);
                level.set_block(portal_block_pos, blocks::END_PORTAL.default_state(), UPDATE_CLIENTS);
            }
        }
        level.global_level_event(SOUND_END_PORTAL_SPAWN, block_pos.offset(1, 0, 1), 0);
    }
    InteractionResult::SUCCESS
}
