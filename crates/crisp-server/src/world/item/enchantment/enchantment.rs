use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_core::random::RandomSource;
use crisp_data::{EquipmentSlot, EquipmentSlotGroup};
use serde_json::Value;

use super::effects::EnchantmentValueEffect;
use crate::core::holder_set::HolderSet;
use crate::world::item::ItemStack;

pub mod components {
    pub const BLOCK_EXPERIENCE: &str = "minecraft:block_experience";
    pub const MOB_EXPERIENCE: &str = "minecraft:mob_experience";
    pub const REPAIR_WITH_XP: &str = "minecraft:repair_with_xp";
}

#[derive(Debug)]
struct ConditionalEffect {
    effect: EnchantmentValueEffect,
    has_requirements: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Cost {
    base: i32,
    per_level_above_first: i32,
}

impl Cost {
    fn parse(v: &Value) -> Cost {
        Cost { base: v["base"].as_i64().unwrap_or(0) as i32, per_level_above_first: v["per_level_above_first"].as_i64().unwrap_or(0) as i32 }
    }

    pub fn calculate(self, level: i32) -> i32 {
        self.base + self.per_level_above_first * (level - 1)
    }
}

#[derive(Debug)]
pub struct EnchantmentDefinition {
    supported_items: HolderSet,
    primary_items: Option<HolderSet>,
    weight: i32,
    max_level: i32,
    min_cost: Cost,
    max_cost: Cost,
}

#[derive(Debug)]
pub struct Enchantment {
    definition: EnchantmentDefinition,
    exclusive_set: HolderSet,
    slots: Vec<EquipmentSlotGroup>,
    effect_components: Vec<String>,
    value_effects: Vec<(&'static str, Vec<ConditionalEffect>)>,
}

fn holder_set(registry: &'static str, v: &Value) -> HolderSet {
    HolderSet::decode(registry, &crate::world::item::component::json_to_nbt(v)).expect("vanilla holder set")
}

fn slot_group(name: &str) -> Option<EquipmentSlotGroup> {
    Some(match name {
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
        _ => return None,
    })
}

impl Enchantment {
    fn parse(v: &Value) -> Enchantment {
        let slots = v["slots"].as_array().into_iter().flatten().filter_map(|s| slot_group(s.as_str()?)).collect();
        let effects = v["effects"].as_object();
        let effect_components = effects.into_iter().flatten().map(|(id, _)| id.clone()).collect();
        let value_effects = [components::BLOCK_EXPERIENCE, components::MOB_EXPERIENCE, components::REPAIR_WITH_XP]
            .into_iter()
            .filter_map(|component| {
                let list = effects?.get(component)?.as_array()?;
                let effects = list
                    .iter()
                    .filter_map(|e| Some(ConditionalEffect { effect: EnchantmentValueEffect::parse(&e["effect"])?, has_requirements: !e["requirements"].is_null() }))
                    .collect();
                Some((component, effects))
            })
            .collect();
        let definition = EnchantmentDefinition {
            supported_items: holder_set("minecraft:item", &v["supported_items"]),
            primary_items: (!v["primary_items"].is_null()).then(|| holder_set("minecraft:item", &v["primary_items"])),
            weight: v["weight"].as_i64().unwrap_or(1) as i32,
            max_level: v["max_level"].as_i64().unwrap_or(1) as i32,
            min_cost: Cost::parse(&v["min_cost"]),
            max_cost: Cost::parse(&v["max_cost"]),
        };
        let exclusive_set = if v["exclusive_set"].is_null() { HolderSet::Direct(Vec::new()) } else { holder_set("minecraft:enchantment", &v["exclusive_set"]) };
        Enchantment { definition, exclusive_set, slots, effect_components, value_effects }
    }

    pub fn is_primary_item(&self, item: &ItemStack) -> bool {
        self.is_supported_item(item) && self.definition.primary_items.as_ref().is_none_or(|primary| primary.contains(item.item.0 as i32))
    }

    pub fn is_supported_item(&self, item: &ItemStack) -> bool {
        self.definition.supported_items.contains(item.item.0 as i32)
    }

    pub fn can_enchant(&self, item: &ItemStack) -> bool {
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

    pub fn decode_holder(tag: &crisp_protocol::nbt::Nbt) -> crate::util::codec::DataResult<String> {
        let id = crate::util::codec::identifier(tag)?;
        crate::world::item::component::codec::registry_id("minecraft:enchantment", &id)?;
        Ok(id)
    }

    pub fn by_id(id: &str) -> Option<&'static Enchantment> {
        static ENCHANTMENTS: OnceLock<HashMap<&'static str, Enchantment>> = OnceLock::new();
        let all = ENCHANTMENTS.get_or_init(|| {
            crisp_data::ENCHANTMENTS.iter().map(|&(id, json)| (id, Enchantment::parse(&serde_json::from_str(json).expect("vanilla JSON")))).collect()
        });
        all.get(id).or_else(|| all.get(crate::commands::arguments::parse_identifier(id)?.as_str()))
    }

    pub fn matching_slot(&self, slot: EquipmentSlot) -> bool {
        self.slots.iter().any(|group| group.test(slot))
    }

    pub fn has_effect(&self, component: &str) -> bool {
        self.effect_components.iter().any(|c| c == component)
    }

    pub fn modify_value(&self, component: &str, level: i32, random: &mut impl RandomSource, value: f32) -> f32 {
        let Some((_, effects)) = self.value_effects.iter().find(|(c, _)| *c == component) else { return value };
        effects.iter().filter(|e| !e.has_requirements).fold(value, |value, e| e.effect.process(level, random, value))
    }
}
