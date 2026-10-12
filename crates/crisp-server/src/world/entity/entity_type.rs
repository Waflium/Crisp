use std::sync::LazyLock;

use crate::world::difficulty::Difficulty;

const FIRE_IMMUNE: [&str; 15] = [
    "minecraft:area_effect_cloud",
    "minecraft:blaze",
    "minecraft:ender_dragon",
    "minecraft:end_crystal",
    "minecraft:ghast",
    "minecraft:magma_cube",
    "minecraft:shulker",
    "minecraft:strider",
    "minecraft:tnt",
    "minecraft:vex",
    "minecraft:warden",
    "minecraft:wither",
    "minecraft:wither_skeleton",
    "minecraft:zoglin",
    "minecraft:zombified_piglin",
];

static FIRE_IMMUNE_IDS: LazyLock<Vec<i32>> =
    LazyLock::new(|| FIRE_IMMUNE.iter().map(|name| crisp_data::registry_id(crisp_data::registry::ENTITY_TYPE, name)).collect());

pub fn fire_immune(entity_type: i32) -> bool {
    FIRE_IMMUNE_IDS.contains(&entity_type)
}

pub struct EntityType {
    allowed_in_peaceful: bool,
}

impl EntityType {
    pub const fn new(allowed_in_peaceful: bool) -> EntityType {
        EntityType { allowed_in_peaceful }
    }

    pub fn can_spawn(&self, difficulty: Difficulty) -> bool {
        self.is_allowed_in_peaceful() || difficulty != Difficulty::Peaceful
    }

    pub fn is_allowed_in_peaceful(&self) -> bool {
        self.allowed_in_peaceful
    }
}
