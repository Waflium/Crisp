use crisp_core::random::RandomSource;
use crisp_core::{Vec3, mth};
use crisp_data::blocks;
use crisp_data::properties::EYE;

use super::ItemStack;
use super::bucket_item::player_pov_hit_result;
use super::item_utils::has_infinite_materials;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::projectile::eye_of_ender::EyeOfEnder;
use crate::world::entity::{LevelEntity, sound_event};
use crate::world::level::clip::ClipFluid;
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

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    if let Some(hit_result) = player_pov_hit_result(level, player, ClipFluid::None)
        && level.block_state(hit_result.block_pos).is_block(blocks::END_PORTAL_FRAME)
    {
        return (InteractionResult::Pass, None);
    }
    level.start_using_item(player, hand);
    let player_pos = level.entity(player).block_position();
    let Some(nearest_map_feature) = level.find_nearest_map_structure("minecraft:eye_of_ender_located", player_pos, 100, false) else {
        return (InteractionResult::CONSUME, None);
    };
    let e = level.entity(player);
    let (x, y, z) = (e.x(), e.y(), e.z());
    let mut eye_of_ender = EyeOfEnder::new(x, y + e.dimensions.height as f64 * 0.5, z);
    eye_of_ender.set_item(&item_stack);
    eye_of_ender.signal_to(Vec3::at_lower_corner_of(nearest_map_feature));
    level.add_fresh_entity(LevelEntity::EyeOfEnder(eye_of_ender));
    let pitch = mth::lerp_f(level.random.next_float(), 0.33, 0.5);
    level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.ender_eye.launch"), SoundSource::Neutral, 1.0, pitch);
    if !has_infinite_materials(level, player) {
        item_stack.shrink(1);
    }
    (InteractionResult::SUCCESS, Some(item_stack))
}
