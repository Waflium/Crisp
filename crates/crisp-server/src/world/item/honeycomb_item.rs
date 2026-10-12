use crisp_data::properties::{CHEST_TYPE, ChestType};
use crisp_data::{BlockState, WAXABLES};

use super::ItemStack;
use crate::server::level::events::PARTICLES_WAX_ON;
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::block::chest_block::get_connected_direction;
use crate::world::level::block::weathering_copper::with_properties_of;
use crate::world::level::update_flags::UPDATE_ALL_IMMEDIATE;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, stack: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let old_state = level.block_state(pos);
    let Some(waxed_state) = get_waxed(old_state) else { return InteractionResult::Pass };
    let mut item_in_hand = stack;
    item_in_hand.shrink(1);
    if level.player(player).game_mode.game_type != GameType::Creative {
        level.player_mut(player).update_item_in_hand(hand, &item_in_hand);
    }
    level.set_block(pos, waxed_state, UPDATE_ALL_IMMEDIATE);
    level.level_event(Some(player), PARTICLES_WAX_ON, pos, 0);
    level.play_sound(Some(player), pos, sound_event("minecraft:item.honeycomb.wax_on"), SoundSource::Blocks, 1.0, 1.0);
    if old_state.block().is_instance_of("ChestBlock") && old_state.get(CHEST_TYPE) != ChestType::Single {
        let neighbor_pos = pos.relative(get_connected_direction(old_state));
        level.level_event(Some(player), PARTICLES_WAX_ON, neighbor_pos, 0);
        level.play_sound(Some(player), pos, sound_event("minecraft:item.honeycomb.wax_on"), SoundSource::Blocks, 1.0, 1.0);
    }
    InteractionResult::SUCCESS
}

pub fn try_apply_to_sign(level: &mut ServerLevel, pos: crisp_core::BlockPos) -> bool {
    if level.sign_set_waxed(pos, true) {
        level.level_event(None, PARTICLES_WAX_ON, pos, 0);
        level.play_sound(None, pos, sound_event("minecraft:item.honeycomb.wax_on"), SoundSource::Blocks, 1.0, 1.0);
        true
    } else {
        false
    }
}

pub fn get_waxed(old_state: BlockState) -> Option<BlockState> {
    WAXABLES.iter().find(|&&(unwaxed, _)| unwaxed == old_state.block()).map(|&(_, waxed)| with_properties_of(waxed, old_state))
}
