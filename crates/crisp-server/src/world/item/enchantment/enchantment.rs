use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_core::random::RandomSource;
use crisp_data::{EquipmentSlot, EquipmentSlotGroup};
use serde_json::Value;

use super::effects::EnchantmentValueEffect;

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

#[derive(Debug, Default)]
pub struct Enchantment {
    slots: Vec<EquipmentSlotGroup>,
    effect_components: Vec<String>,
    value_effects: Vec<(&'static str, Vec<ConditionalEffect>)>,
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
        Enchantment { slots, effect_components, value_effects }
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
