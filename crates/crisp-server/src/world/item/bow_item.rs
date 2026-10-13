use crisp_core::random::RandomSource;

use super::ItemStack;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, sound_event};
use crate::world::{InteractionHand, InteractionResult};

pub const USE_DURATION: i32 = 72000;

pub fn release_using(level: &mut ServerLevel, item_stack: &ItemStack, entity: EntityId, remaining_time: i32) -> bool {
    if !level.is_player(entity) {
        return false;
    }
    let player = entity;
    let projectile = level.get_projectile(player, item_stack);
    if projectile.is_empty() {
        return false;
    }
    let time_held = USE_DURATION - remaining_time;
    let pow = get_power_for_time(time_held);
    if (pow as f64) < 0.1 {
        return false;
    }
    let fired_projectiles = level.draw(item_stack, projectile, player);
    if !fired_projectiles.is_empty() {
        let hand = level.used_item_hand(player);
        level.projectile_weapon_shoot(player, hand, &fired_projectiles, pow * 3.0, 1.0, pow == 1.0, None);
    }
    let e = level.entity(player);
    let (x, y, z) = (e.x(), e.y(), e.z());
    let pitch = 1.0 / (level.random.next_float() * 0.4 + 1.2) + pow * 0.5;
    level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.arrow.shoot"), SoundSource::Players, 1.0, pitch);
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.get_item().0), 1);
    true
}

pub fn get_power_for_time(time_held: i32) -> f32 {
    let mut pow = time_held as f32 / 20.0;
    pow = (pow * pow + pow * 2.0) / 3.0;
    if pow > 1.0 {
        pow = 1.0;
    }
    pow
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let found_projectile = !level.get_projectile(player, &item_stack).is_empty();
    if !level.has_infinite_materials(player) && !found_projectile {
        return (InteractionResult::Fail, None);
    }
    level.start_using_item(player, hand);
    (InteractionResult::CONSUME, None)
}
