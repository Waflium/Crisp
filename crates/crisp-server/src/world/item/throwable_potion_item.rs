use crisp_core::Vec3;
use crisp_core::random::RandomSource;
use crisp_data::Item;

use super::ItemStack;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::entity::projectile::throwableitemprojectile::thrown_lingering_potion::ThrownLingeringPotion;
use crate::world::entity::projectile::throwableitemprojectile::thrown_splash_potion::ThrownSplashPotion;
use crate::world::entity::{EntityId, sound_event};
use crate::world::{InteractionHand, InteractionResult};

pub const PROJECTILE_SHOOT_POWER: f32 = 0.5;

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let item = item_stack.get_item();
    let e = level.entity(player);
    let (x, y, z) = (e.x(), e.y(), e.z());
    let pitch = 0.4 / (level.random.next_float() * 0.4 + 0.8);
    let (sound, source) = if item.is_instance_of("LingeringPotionItem") {
        ("minecraft:entity.lingering_potion.throw", SoundSource::Neutral)
    } else {
        ("minecraft:entity.splash_potion.throw", SoundSource::Players)
    };
    level.play_sound_at(None, x, y, z, sound_event(sound), source, 0.5, pitch);
    let projectile = create_potion(level, item, player, &item_stack);
    level.spawn_projectile_from_rotation(projectile, &item_stack, player, -20.0, PROJECTILE_SHOOT_POWER, 1.0);
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item.0), 1);
    item_stack.consume(1, level, Some(player));
    (InteractionResult::SUCCESS, Some(item_stack))
}

fn create_potion(level: &ServerLevel, item: Item, owner: EntityId, item_stack: &ItemStack) -> Projectile {
    if item.is_instance_of("LingeringPotionItem") { ThrownLingeringPotion::from_owner(level, owner, item_stack) } else { ThrownSplashPotion::from_owner(level, owner, item_stack) }
}

pub fn as_projectile(item: Item, position: Vec3, item_stack: &ItemStack) -> Projectile {
    if item.is_instance_of("LingeringPotionItem") {
        ThrownLingeringPotion::at(position.x, position.y, position.z, item_stack)
    } else {
        ThrownSplashPotion::at(position.x, position.y, position.z, item_stack)
    }
}
