use crisp_core::mth;

use super::DamageSource;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub const MAX_ARMOR: f32 = 20.0;
pub const ARMOR_PROTECTION_DIVIDER: f32 = 25.0;
pub const BASE_ARMOR_TOUGHNESS: f32 = 2.0;
pub const MIN_ARMOR_RATIO: f32 = 0.2;

pub fn get_damage_after_absorb(level: &mut ServerLevel, victim: EntityId, damage: f32, source: &DamageSource, total_armor: f32, armor_toughness: f32) -> f32 {
    let toughness = BASE_ARMOR_TOUGHNESS + armor_toughness / 4.0;
    let real_armor = mth::clamp_f(total_armor - damage / toughness, total_armor * MIN_ARMOR_RATIO, MAX_ARMOR);
    let armor_fraction = real_armor / ARMOR_PROTECTION_DIVIDER;
    let modified_armor_fraction = match source.get_weapon_item(level) {
        Some(weapon_item) => mth::clamp_f(level.modify_armor_effectiveness(&weapon_item, victim, source, armor_fraction), 0.0, 1.0),
        None => armor_fraction,
    };
    let damage_multiplier = 1.0 - modified_armor_fraction;
    damage * damage_multiplier
}

pub fn get_damage_after_magic_absorb(damage: f32, total_magic_armor: f32) -> f32 {
    let real_armor = mth::clamp_f(total_magic_armor, 0.0, MAX_ARMOR);
    damage * (1.0 - real_armor / ARMOR_PROTECTION_DIVIDER)
}
