use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::{LIT, WATERLOGGED};
use crisp_data::{BlockState, EquipmentSlot, block_tags};

use super::context::BlockPlaceContext;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::entity::movement::sound_event;
use crate::world::level::BlockGetter;
use crate::world::level::block::base_fire_block;
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_ALL_IMMEDIATE};
use crate::world::{InteractionHand, InteractionResult};

fn can_light(state: BlockState) -> bool {
    let dry_and_unlit = |state: BlockState| state.has(LIT) && state.has(WATERLOGGED) && !state.get(LIT) && !state.get(WATERLOGGED);
    (state.is(&block_tags::CAMPFIRES) && dry_and_unlit(state))
        || (state.is(&block_tags::CANDLES) && dry_and_unlit(state))
        || (state.is(&block_tags::CANDLE_CAKES) && state.has(LIT) && !state.get(LIT))
}

fn equipment_slot(hand: InteractionHand) -> EquipmentSlot {
    match hand {
        InteractionHand::MainHand => EquipmentSlot::Mainhand,
        InteractionHand::OffHand => EquipmentSlot::Offhand,
    }
}

pub fn flint_and_steel_use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, ctx: &BlockPlaceContext) -> InteractionResult {
    let pos = ctx.hit.block_pos;
    let state = level.block_state(pos);
    let target = if can_light(state) {
        pos
    } else {
        let relative_pos = pos.relative(ctx.clicked_face());
        if !base_fire_block::can_be_placed_at(level, relative_pos) {
            return InteractionResult::Fail;
        }
        relative_pos
    };
    let pitch = level.random.next_float() * 0.4 + 0.8;
    level.play_sound(Some(player), target, sound_event("minecraft:item.flintandsteel.use"), SoundSource::Blocks, 1.0, pitch);
    let new_state = if target == pos { state.set(LIT, true) } else { base_fire_block::get_state(level, target) };
    level.set_block(target, new_state, UPDATE_ALL_IMMEDIATE);
    let slot = level.player(player).hand_slot(hand);
    level.hurt_and_break(player, slot, 1, equipment_slot(hand));
    InteractionResult::SUCCESS
}

pub fn fire_charge_use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, ctx: &BlockPlaceContext) -> InteractionResult {
    let mut pos = ctx.hit.block_pos;
    let state = level.block_state(pos);
    if can_light(state) {
        play_fire_charge_sound(level, pos);
        level.set_block(pos, state.set(LIT, true), UPDATE_ALL);
    } else {
        pos = pos.relative(ctx.clicked_face());
        if !base_fire_block::can_be_placed_at(level, pos) {
            return InteractionResult::Fail;
        }
        play_fire_charge_sound(level, pos);
        let fire = base_fire_block::get_state(level, pos);
        level.set_block(pos, fire, UPDATE_ALL);
    }
    let p = level.player_mut(player);
    let slot = p.hand_slot(hand);
    if let Some(held) = &mut p.inventory.slots[slot] {
        held.count -= 1;
        if held.count <= 0 {
            p.inventory.slots[slot] = None;
        }
    }
    p.dirty_slots.push(slot);
    InteractionResult::SUCCESS
}

fn play_fire_charge_sound(level: &mut ServerLevel, pos: BlockPos) {
    let pitch = (level.random.next_float() - level.random.next_float()) * 0.2 + 1.0;
    level.play_sound(None, pos, sound_event("minecraft:item.firecharge.use"), SoundSource::Blocks, 1.0, pitch);
}
