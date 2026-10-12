use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_core::Vec3;
use crisp_data::{EquipmentSlot, EquipmentSlotGroup};
use crisp_protocol::nbt::Nbt;

use super::conditional_effect::ConditionalEffect;
use super::effects::enchantment_entity_effect::EnchantmentEntityEffect;
use super::effects::enchantment_value_effect::EnchantmentValueEffect;
use super::enchanted_item_in_use::EnchantedItemInUse;
use super::enchantment_effect_components::EnchantmentEffectComponents;
use super::targeted_conditional_effect::{EnchantmentTarget, TargetedConditionalEffect};
use crate::core::holder::RegistryElement;
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::LootParams;

pub const MAX_LEVEL: i32 = 255;

#[derive(Clone, Copy, Debug, Default)]
pub struct Cost {
    base: i32,
    per_level_above_first: i32,
}

impl Cost {
    fn decode(tag: &Nbt) -> DataResult<Cost> {
        Ok(Cost { base: codec::decode_field(tag, "base", codec::int)?, per_level_above_first: codec::decode_field(tag, "per_level_above_first", codec::int)? })
    }

    pub fn calculate(self, level: i32) -> i32 {
        self.base + self.per_level_above_first * (level - 1)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct EnchantmentDefinition {
    supported_items: HolderSet,
    primary_items: Option<HolderSet>,
    weight: i32,
    max_level: i32,
    min_cost: Cost,
    max_cost: Cost,
    anvil_cost: i32,
    slots: Vec<EquipmentSlotGroup>,
}

fn int_range(tag: &Nbt, min: i32, max: i32) -> DataResult<i32> {
    let value = codec::int(tag)?;
    if value < min || value > max {
        return Err(format!("Value {value} outside of range [{min}:{max}]"));
    }
    Ok(value)
}

pub fn slot_group(tag: &Nbt) -> DataResult<EquipmentSlotGroup> {
    let name = codec::string(tag)?;
    Ok(match name {
        "any" => EquipmentSlotGroup::Any,
        "mainhand" => EquipmentSlotGroup::Mainhand,
        "offhand" => EquipmentSlotGroup::Offhand,
        "hand" => EquipmentSlotGroup::Hand,
        "feet" => EquipmentSlotGroup::Feet,
        "legs" => EquipmentSlotGroup::Legs,
        "chest" => EquipmentSlotGroup::Chest,
        "head" => EquipmentSlotGroup::Head,
        "armor" => EquipmentSlotGroup::Armor,
        "body" => EquipmentSlotGroup::Body,
        "saddle" => EquipmentSlotGroup::Saddle,
        _ => return Err(format!("Unknown element name:{name}")),
    })
}

impl EnchantmentDefinition {
    fn decode(tag: &Nbt) -> DataResult<EnchantmentDefinition> {
        Ok(EnchantmentDefinition {
            supported_items: codec::decode_field(tag, "supported_items", |t| HolderSet::decode("minecraft:item", t))?,
            primary_items: codec::decode_optional(tag, "primary_items", |t| HolderSet::decode("minecraft:item", t))?,
            weight: codec::decode_field(tag, "weight", |t| int_range(t, 1, 1024))?,
            max_level: codec::decode_field(tag, "max_level", |t| int_range(t, 1, MAX_LEVEL))?,
            min_cost: codec::decode_field(tag, "min_cost", Cost::decode)?,
            max_cost: codec::decode_field(tag, "max_cost", Cost::decode)?,
            anvil_cost: codec::decode_field(tag, "anvil_cost", |t| int_range(t, 0, i32::MAX))?,
            slots: codec::decode_field(tag, "slots", |t| codec::list_of(t, slot_group))?,
        })
    }
}

#[allow(dead_code)]
pub struct Enchantment {
    description: Nbt,
    definition: EnchantmentDefinition,
    exclusive_set: HolderSet,
    pub effects: EnchantmentEffectComponents,
}

impl RegistryElement for Enchantment {
    const REGISTRY: &'static str = "minecraft:enchantment";

    fn contains_key(key: &str) -> bool {
        crisp_data::ENCHANTMENTS.iter().any(|(name, _)| *name == key)
    }

    fn lookup(key: &str) -> Option<&'static Enchantment> {
        static ENCHANTMENTS: OnceLock<HashMap<&'static str, Enchantment>> = OnceLock::new();
        ENCHANTMENTS
            .get_or_init(|| {
                crisp_data::ENCHANTMENTS
                    .iter()
                    .map(|&(name, json)| {
                        let tag = crate::world::item::component::json_to_nbt(&serde_json::from_str(json).expect("vanilla JSON"));
                        (name, Enchantment::decode(&tag).unwrap_or_else(|e| panic!("{name}: {e}")))
                    })
                    .collect()
            })
            .get(key)
    }
}

fn damage_context<'a>(level: &'a mut ServerLevel, enchantment_level: i32, victim: EntityId, source: &DamageSource) -> LootContext<'a> {
    let position = level.entity(victim).position();
    let mut params = LootParams::new(level);
    params.this_entity = Some(victim);
    params.enchantment_level = Some(enchantment_level);
    params.origin = Some(position);
    params.damage_source = Some(*source);
    params.attacking_entity = source.causing_entity;
    params.direct_attacking_entity = source.direct_entity;
    LootContextBuilder::new(params).create(None)
}

fn item_context<'a>(level: &'a mut ServerLevel, enchantment_level: i32, item_stack: &'a ItemStack) -> LootContext<'a> {
    let mut params = LootParams::new(level);
    params.tool = Some(item_stack);
    params.enchantment_level = Some(enchantment_level);
    LootContextBuilder::new(params).create(None)
}

fn entity_context(level: &mut ServerLevel, enchantment_level: i32, entity: EntityId, position: Vec3) -> LootContext<'_> {
    let mut params = LootParams::new(level);
    params.this_entity = Some(entity);
    params.enchantment_level = Some(enchantment_level);
    params.origin = Some(position);
    LootContextBuilder::new(params).create(None)
}

impl Enchantment {
    fn decode(tag: &Nbt) -> DataResult<Enchantment> {
        Ok(Enchantment {
            description: codec::decode_field(tag, "description", |t| Ok(t.clone()))?,
            definition: EnchantmentDefinition::decode(tag)?,
            exclusive_set: codec::decode_or(tag, "exclusive_set", HolderSet::Direct(Vec::new()), |t| HolderSet::decode("minecraft:enchantment", t))?,
            effects: codec::decode_or(tag, "effects", EnchantmentEffectComponents::default(), EnchantmentEffectComponents::decode)?,
        })
    }

    pub fn by_id(id: &str) -> Option<&'static Enchantment> {
        Enchantment::lookup(id).or_else(|| Enchantment::lookup(&crate::commands::arguments::parse_identifier(id)?))
    }

    pub fn decode_holder(tag: &Nbt) -> DataResult<String> {
        let id = codec::identifier(tag)?;
        crate::world::item::component::codec::registry_id("minecraft:enchantment", &id)?;
        Ok(id)
    }

    pub fn matching_slot(&self, slot: EquipmentSlot) -> bool {
        self.definition.slots.iter().any(|group| group.test(slot))
    }

    pub fn is_primary_item(&self, item: &ItemStack) -> bool {
        self.is_supported_item(item) && self.definition.primary_items.as_ref().is_none_or(|primary| primary.contains(item.item.0 as i32))
    }

    pub fn is_supported_item(&self, item: &ItemStack) -> bool {
        self.definition.supported_items.contains(item.item.0 as i32)
    }

    pub fn weight(&self) -> i32 {
        self.definition.weight
    }

    pub fn min_level(&self) -> i32 {
        1
    }

    pub fn max_level(&self) -> i32 {
        self.definition.max_level
    }

    pub fn get_anvil_cost(&self) -> i32 {
        self.definition.anvil_cost
    }

    pub fn min_cost(&self, level: i32) -> i32 {
        self.definition.min_cost.calculate(level)
    }

    pub fn max_cost(&self, level: i32) -> i32 {
        self.definition.max_cost.calculate(level)
    }

    pub fn are_compatible(enchantment: &str, other: &str) -> bool {
        let id = |name: &str| crate::world::item::component::codec::registry_id("minecraft:enchantment", name).unwrap_or(-1);
        let (Some(a), Some(b)) = (Enchantment::by_id(enchantment), Enchantment::by_id(other)) else { return enchantment != other };
        enchantment != other && !a.exclusive_set.contains(id(other)) && !b.exclusive_set.contains(id(enchantment))
    }

    pub fn can_enchant(&self, item_stack: &ItemStack) -> bool {
        self.definition.supported_items.contains(item_stack.item.0 as i32)
    }

    pub fn is_immune_to_damage(&self, level: &mut ServerLevel, enchantment_level: i32, victim: EntityId, source: &DamageSource) -> bool {
        let mut context = damage_context(level, enchantment_level, victim, source);
        self.effects.damage_immunity.iter().any(|effect| effect.matches(&mut context))
    }

    pub fn modify_damage_protection(&self, level: &mut ServerLevel, enchantment_level: i32, victim: EntityId, source: &DamageSource, protection: &mut f32) {
        modify_damage_filtered_value(&self.effects.damage_protection, level, enchantment_level, victim, source, protection);
    }

    pub fn modify_durability_change(&self, level: &mut ServerLevel, enchantment_level: i32, item_stack: &ItemStack, change: &mut f32) {
        modify_item_filtered_count(&self.effects.item_damage, level, enchantment_level, item_stack, change);
    }

    pub fn modify_block_experience(&self, level: &mut ServerLevel, enchantment_level: i32, item_stack: &ItemStack, count: &mut f32) {
        modify_item_filtered_count(&self.effects.block_experience, level, enchantment_level, item_stack, count);
    }

    pub fn modify_mob_experience(&self, level: &mut ServerLevel, enchantment_level: i32, killer: EntityId, experience: &mut f32) {
        modify_entity_filtered_value(&self.effects.mob_experience, level, enchantment_level, killer, experience);
    }

    pub fn modify_durability_to_repair_from_xp(&self, level: &mut ServerLevel, enchantment_level: i32, item_stack: &ItemStack, change: &mut f32) {
        modify_item_filtered_count(&self.effects.repair_with_xp, level, enchantment_level, item_stack, change);
    }

    pub fn modify_damage(&self, level: &mut ServerLevel, enchantment_level: i32, victim: EntityId, source: &DamageSource, amount: &mut f32) {
        modify_damage_filtered_value(&self.effects.damage, level, enchantment_level, victim, source, amount);
    }

    pub fn modify_fall_based_damage(&self, level: &mut ServerLevel, enchantment_level: i32, victim: EntityId, source: &DamageSource, amount: &mut f32) {
        modify_damage_filtered_value(&self.effects.smash_damage_per_fallen_block, level, enchantment_level, victim, source, amount);
    }

    pub fn modify_knockback(&self, level: &mut ServerLevel, enchantment_level: i32, victim: EntityId, source: &DamageSource, amount: &mut f32) {
        modify_damage_filtered_value(&self.effects.knockback, level, enchantment_level, victim, source, amount);
    }

    pub fn modify_armor_effectiveness(&self, level: &mut ServerLevel, enchantment_level: i32, victim: EntityId, source: &DamageSource, amount: &mut f32) {
        modify_damage_filtered_value(&self.effects.armor_effectiveness, level, enchantment_level, victim, source, amount);
    }

    pub fn do_post_attack(
        &self,
        level: &mut ServerLevel,
        enchantment_level: i32,
        item: &EnchantedItemInUse,
        for_target: EnchantmentTarget,
        victim: EntityId,
        source: &DamageSource,
    ) {
        for effect in &self.effects.post_attack {
            if for_target == effect.enchanted {
                do_post_attack(effect, level, enchantment_level, item, victim, source);
            }
        }
    }

    pub fn do_post_piercing_attack(&self, level: &mut ServerLevel, enchantment_level: i32, item: &EnchantedItemInUse, user: EntityId) {
        let position = level.entity(user).position();
        for effect in &self.effects.post_piercing_attack {
            let matches = effect.matches(&mut entity_context(level, enchantment_level, user, position));
            if matches {
                effect.effect.apply(level, enchantment_level, item, user, position);
            }
        }
    }
}

fn do_post_attack(
    effect: &TargetedConditionalEffect<EnchantmentEntityEffect>,
    level: &mut ServerLevel,
    enchantment_level: i32,
    item: &EnchantedItemInUse,
    victim: EntityId,
    source: &DamageSource,
) {
    if effect.matches(&mut damage_context(level, enchantment_level, victim, source)) {
        let target = match effect.affected {
            EnchantmentTarget::Attacker => source.causing_entity,
            EnchantmentTarget::DamagingEntity => source.direct_entity,
            EnchantmentTarget::Victim => Some(victim),
        };
        if let Some(target) = target {
            let position = level.entity(target).position();
            effect.effect.apply(level, enchantment_level, item, target, position);
        }
    }
}

fn modify_item_filtered_count(
    effects: &[ConditionalEffect<EnchantmentValueEffect>],
    level: &mut ServerLevel,
    enchantment_level: i32,
    item_stack: &ItemStack,
    value: &mut f32,
) {
    if effects.is_empty() {
        return;
    }
    let mut context = item_context(level, enchantment_level, item_stack);
    for effect in effects {
        if effect.matches(&mut context) {
            *value = effect.effect.process(enchantment_level, &mut context.random(), *value);
        }
    }
}

fn modify_entity_filtered_value(effects: &[ConditionalEffect<EnchantmentValueEffect>], level: &mut ServerLevel, enchantment_level: i32, entity: EntityId, value: &mut f32) {
    if effects.is_empty() {
        return;
    }
    let position = level.entity(entity).position();
    let mut context = entity_context(level, enchantment_level, entity, position);
    for effect in effects {
        if effect.matches(&mut context) {
            *value = effect.effect.process(enchantment_level, &mut context.level().entity_mut(entity).random, *value);
        }
    }
}

fn modify_damage_filtered_value(
    effects: &[ConditionalEffect<EnchantmentValueEffect>],
    level: &mut ServerLevel,
    enchantment_level: i32,
    victim: EntityId,
    source: &DamageSource,
    value: &mut f32,
) {
    if effects.is_empty() {
        return;
    }
    let mut context = damage_context(level, enchantment_level, victim, source);
    for effect in effects {
        if effect.matches(&mut context) {
            *value = effect.effect.process(enchantment_level, &mut context.level().entity_mut(victim).random, *value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Enchantment;
    use crate::world::item::component::json_to_nbt;

    #[test]
    fn every_vanilla_enchantment_decodes() {
        let errors: Vec<String> = crisp_data::ENCHANTMENTS
            .iter()
            .filter_map(|&(name, json)| Enchantment::decode(&json_to_nbt(&serde_json::from_str(json).unwrap())).err().map(|e| format!("{name}: {e}")))
            .collect();
        assert!(errors.is_empty(), "{errors:#?}");
    }
}
