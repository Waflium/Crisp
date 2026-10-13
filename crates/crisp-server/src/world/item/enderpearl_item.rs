use crisp_core::random::RandomSource;

use super::ItemStack;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::projectile::throwableitemprojectile::thrown_enderpearl::ThrownEnderpearl;
use crate::world::entity::{EntityId, sound_event};
use crate::world::{InteractionHand, InteractionResult};

pub const PROJECTILE_SHOOT_POWER: f32 = 1.5;

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let e = level.entity(player);
    let (x, y, z) = (e.x(), e.y(), e.z());
    let pitch = 0.4 / (level.random.next_float() * 0.4 + 0.8);
    level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.ender_pearl.throw"), SoundSource::Neutral, 0.5, pitch);
    let projectile = ThrownEnderpearl::from_owner(level, player, &item_stack);
    level.spawn_projectile_from_rotation(projectile, &item_stack, player, 0.0, PROJECTILE_SHOOT_POWER, 1.0);
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.get_item().0), 1);
    item_stack.consume(1, level, Some(player));
    (InteractionResult::SUCCESS, Some(item_stack))
}
