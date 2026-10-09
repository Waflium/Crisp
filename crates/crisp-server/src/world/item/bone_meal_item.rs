use crisp_core::BlockPos;

use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_PLANT_GROWTH;
use crate::server::level::game_mode::GameType;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, BlockHitResult};
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult) -> InteractionResult {
    let pos = hit.block_pos;
    if grow_crop(level, Some((player, hand)), pos) {
        level.level_event(None, PARTICLES_AND_SOUND_PLANT_GROWTH, pos, 15);
        return InteractionResult::SUCCESS_SERVER;
    }
    InteractionResult::Pass
}

pub fn grow_crop(level: &mut ServerLevel, user: Option<(EntityId, InteractionHand)>, pos: BlockPos) -> bool {
    let state = level.block_state(pos);
    let behaviour = block::of(state);
    if !behaviour.is_valid_bonemeal_target(level, pos, state) {
        return false;
    }
    if behaviour.is_bonemeal_success(level, pos, state) {
        behaviour.perform_bonemeal(level, pos, state);
    }
    if let Some((player, hand)) = user {
        let p = level.player_mut(player);
        if p.game_mode.game_type != GameType::Creative {
            let slot = p.hand_slot(hand);
            if let Some(held) = &mut p.inventory.slots[slot] {
                held.count -= 1;
                if held.count <= 0 {
                    p.inventory.slots[slot] = None;
                }
            }
            p.dirty_slots.push(slot);
        }
    }
    true
}
