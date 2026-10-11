use crisp_protocol::nbt::Nbt;

use super::conditional_effect::ConditionalEffect;
use super::effects::enchantment_attribute_effect::EnchantmentAttributeEffect;
use super::effects::enchantment_entity_effect::EnchantmentEntityEffect;
use super::effects::enchantment_location_based_effect::EnchantmentLocationBasedEffect;
use super::effects::enchantment_value_effect::EnchantmentValueEffect;
use super::targeted_conditional_effect::TargetedConditionalEffect;
use crate::util::codec::{self, DataResult};

pub const REPAIR_WITH_XP: &str = "minecraft:repair_with_xp";

#[allow(dead_code)]
#[derive(Default)]
pub struct EnchantmentEffectComponents {
    pub components: Vec<String>,
    pub damage_protection: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub damage_immunity: Vec<ConditionalEffect<()>>,
    pub damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub smash_damage_per_fallen_block: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub knockback: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub armor_effectiveness: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub post_attack: Vec<TargetedConditionalEffect<EnchantmentEntityEffect>>,
    pub post_piercing_attack: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub hit_block: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub item_damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub equipment_drops: Vec<TargetedConditionalEffect<EnchantmentValueEffect>>,
    pub location_changed: Vec<ConditionalEffect<EnchantmentLocationBasedEffect>>,
    pub tick: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub ammo_use: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub projectile_piercing: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub projectile_spawned: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub projectile_spread: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub projectile_count: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub trident_return_acceleration: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub fishing_time_reduction: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub fishing_luck_bonus: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub block_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub mob_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub repair_with_xp: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub attributes: Vec<EnchantmentAttributeEffect>,
    pub crossbow_charge_time: Option<EnchantmentValueEffect>,
    pub crossbow_charging_sounds: Vec<Nbt>,
    pub trident_sound: Vec<Nbt>,
    pub prevent_equipment_drop: bool,
    pub prevent_armor_change: bool,
    pub trident_spin_attack_strength: Option<EnchantmentValueEffect>,
}

fn value_effects(tag: &Nbt) -> DataResult<Vec<ConditionalEffect<EnchantmentValueEffect>>> {
    codec::list_of(tag, |t| ConditionalEffect::decode(t, EnchantmentValueEffect::decode))
}

fn entity_effects(tag: &Nbt) -> DataResult<Vec<ConditionalEffect<EnchantmentEntityEffect>>> {
    codec::list_of(tag, |t| ConditionalEffect::decode(t, EnchantmentEntityEffect::decode))
}

impl EnchantmentEffectComponents {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentEffectComponents> {
        let mut effects = EnchantmentEffectComponents::default();
        for (key, value) in codec::map(tag)? {
            let id = codec::identifier(&Nbt::String(key.clone()))?;
            match id.as_str() {
                "minecraft:damage_protection" => effects.damage_protection = value_effects(value)?,
                "minecraft:damage_immunity" => {
                    effects.damage_immunity = codec::list_of(value, |t| ConditionalEffect::decode(t, |e| codec::map(e).map(|_| ())))?;
                }
                "minecraft:damage" => effects.damage = value_effects(value)?,
                "minecraft:smash_damage_per_fallen_block" => effects.smash_damage_per_fallen_block = value_effects(value)?,
                "minecraft:knockback" => effects.knockback = value_effects(value)?,
                "minecraft:armor_effectiveness" => effects.armor_effectiveness = value_effects(value)?,
                "minecraft:post_attack" => {
                    effects.post_attack = codec::list_of(value, |t| TargetedConditionalEffect::decode(t, EnchantmentEntityEffect::decode))?;
                }
                "minecraft:post_piercing_attack" => effects.post_piercing_attack = entity_effects(value)?,
                "minecraft:hit_block" => effects.hit_block = entity_effects(value)?,
                "minecraft:item_damage" => effects.item_damage = value_effects(value)?,
                "minecraft:equipment_drops" => {
                    effects.equipment_drops = codec::list_of(value, |t| TargetedConditionalEffect::decode_equipment_drops(t, EnchantmentValueEffect::decode))?;
                }
                "minecraft:location_changed" => {
                    effects.location_changed = codec::list_of(value, |t| ConditionalEffect::decode(t, EnchantmentLocationBasedEffect::decode))?;
                }
                "minecraft:tick" => effects.tick = entity_effects(value)?,
                "minecraft:ammo_use" => effects.ammo_use = value_effects(value)?,
                "minecraft:projectile_piercing" => effects.projectile_piercing = value_effects(value)?,
                "minecraft:projectile_spawned" => effects.projectile_spawned = entity_effects(value)?,
                "minecraft:projectile_spread" => effects.projectile_spread = value_effects(value)?,
                "minecraft:projectile_count" => effects.projectile_count = value_effects(value)?,
                "minecraft:trident_return_acceleration" => effects.trident_return_acceleration = value_effects(value)?,
                "minecraft:fishing_time_reduction" => effects.fishing_time_reduction = value_effects(value)?,
                "minecraft:fishing_luck_bonus" => effects.fishing_luck_bonus = value_effects(value)?,
                "minecraft:block_experience" => effects.block_experience = value_effects(value)?,
                "minecraft:mob_experience" => effects.mob_experience = value_effects(value)?,
                "minecraft:repair_with_xp" => effects.repair_with_xp = value_effects(value)?,
                "minecraft:attributes" => effects.attributes = codec::list_of(value, EnchantmentAttributeEffect::decode)?,
                "minecraft:crossbow_charge_time" => effects.crossbow_charge_time = Some(EnchantmentValueEffect::decode(value)?),
                "minecraft:crossbow_charging_sounds" => effects.crossbow_charging_sounds = codec::list(value)?,
                "minecraft:trident_sound" => effects.trident_sound = codec::list(value)?,
                "minecraft:prevent_equipment_drop" => effects.prevent_equipment_drop = true,
                "minecraft:prevent_armor_change" => effects.prevent_armor_change = true,
                "minecraft:trident_spin_attack_strength" => effects.trident_spin_attack_strength = Some(EnchantmentValueEffect::decode(value)?),
                other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_effect_component_type]: {other}")),
            }
            effects.components.push(id);
        }
        Ok(effects)
    }

    pub fn has(&self, component: &str) -> bool {
        self.components.iter().any(|c| c == component)
    }
}
