use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::{SyncedRegistry, registry, registry_id};
use serde_json::Value;

use super::Animal;
use crate::server::level::ServerLevel;
use crate::world::entity::attributes::CHICKEN_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::movement::sound_event;
use crate::world::entity::{Entity, EntityDimensions, EntityId};

pub const DATA_VARIANT: usize = 18;
const CHICKEN_LAY: &str = "minecraft:gameplay/chicken_lay";
pub const DATA_SOUND_VARIANT: usize = 19;
pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.4, height: 0.7, eye_height: 0.644 };
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.3, height: 0.4, eye_height: 0.28125 };

#[derive(Default)]
pub struct Chicken {
    pub animal: Animal,
    pub egg_time: i32,
    pub is_chicken_jockey: bool,
}

pub struct SoundSet {
    pub ambient_sound: String,
    pub death_sound: String,
    pub hurt_sound: String,
    pub step_sound: String,
}

pub struct ChickenSoundVariant {
    pub adult_sounds: SoundSet,
    pub baby_sounds: SoundSet,
}

struct SpawnSelector {
    biomes: Option<Vec<i32>>,
    priority: i32,
}

fn variants() -> &'static SyncedRegistry {
    SyncedRegistry::get("minecraft:chicken_variant")
}

fn sound_variants() -> &'static SyncedRegistry {
    SyncedRegistry::get("minecraft:chicken_sound_variant")
}

fn spawn_selectors() -> &'static Vec<(i32, SpawnSelector)> {
    static SELECTORS: OnceLock<Vec<(i32, SpawnSelector)>> = OnceLock::new();
    SELECTORS.get_or_init(|| {
        let json: HashMap<&str, Value> = crisp_data::CHICKEN_VARIANTS.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        let mut selectors = Vec::new();
        for (id, name) in variants().entries.iter().enumerate() {
            for selector in json[name]["spawn_conditions"].as_array().into_iter().flatten() {
                let biomes = selector.get("condition").map(|condition| {
                    let biomes = condition["biomes"].as_str().unwrap_or_default();
                    match biomes.strip_prefix('#') {
                        Some(tag) => crisp_data::tag("minecraft:worldgen/biome", tag).to_vec(),
                        None => SyncedRegistry::get("minecraft:worldgen/biome").id_of(biomes).into_iter().collect(),
                    }
                });
                selectors.push((id as i32, SpawnSelector { biomes, priority: selector["priority"].as_i64().unwrap_or(0) as i32 }));
            }
        }
        selectors
    })
}

pub fn sound_variant(id: i32) -> &'static ChickenSoundVariant {
    static SOUND_VARIANTS: OnceLock<Vec<ChickenSoundVariant>> = OnceLock::new();
    let all = SOUND_VARIANTS.get_or_init(|| {
        let json: HashMap<&str, Value> = crisp_data::CHICKEN_SOUND_VARIANTS.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        let set = |v: &Value| SoundSet {
            ambient_sound: v["ambient_sound"].as_str().unwrap_or_default().to_string(),
            death_sound: v["death_sound"].as_str().unwrap_or_default().to_string(),
            hurt_sound: v["hurt_sound"].as_str().unwrap_or_default().to_string(),
            step_sound: v["step_sound"].as_str().unwrap_or_default().to_string(),
        };
        sound_variants().entries.iter().map(|name| ChickenSoundVariant { adult_sounds: set(&json[name]["adult_sounds"]), baby_sounds: set(&json[name]["baby_sounds"]) }).collect()
    });
    &all[id as usize]
}

pub fn variant_id(name: &str) -> Option<i32> {
    variants().id_of(name)
}

pub fn variant_name(id: i32) -> &'static str {
    variants().entries[id as usize]
}

pub fn sound_variant_id(name: &str) -> Option<i32> {
    sound_variants().id_of(name)
}

pub fn sound_variant_name(id: i32) -> &'static str {
    sound_variants().entries[id as usize]
}

impl Chicken {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        let temperate = variant_id("minecraft:temperate").unwrap_or(0);
        let classic = sound_variant_id("minecraft:classic").unwrap_or(0);
        data.extend([DataValue::ChickenVariant(temperate), DataValue::ChickenSoundVariant(classic)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:chicken"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&CHICKEN_ATTRIBUTES), MobKind::Chicken(Chicken::default()));
        let egg_time = mob.entity.random.next_int_bounded(6000) + 6000;
        let MobKind::Chicken(chicken) = &mut mob.kind;
        chicken.egg_time = egg_time;
        mob
    }
}

impl ServerLevel {
    pub fn chicken_ai_step(&mut self, id: EntityId) {
        self.animal_ai_step(id);
        if self.mob(id).is_none() {
            return;
        }
        let entity = self.entity_mut(id);
        let movement = entity.delta_movement;
        if !entity.on_ground && movement.y < 0.0 {
            entity.delta_movement = Vec3::new(movement.x, movement.y * 0.6, movement.z);
        }
        let alive = self.is_alive(id);
        let mob = self.mob_mut(id).expect("a mob");
        let baby = mob.is_baby();
        let MobKind::Chicken(chicken) = &mut mob.kind;
        if alive && !baby && !chicken.is_chicken_jockey && {
            chicken.egg_time -= 1;
            chicken.egg_time <= 0
        } {
            if self.drop_from_gift_loot_table(id, CHICKEN_LAY) {
                let random = &mut self.entity_mut(id).random;
                let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
                self.entity_play_sound(id, sound_event("minecraft:entity.chicken.egg"), 1.0, pitch);
            }
            let mob = self.mob_mut(id).expect("a mob");
            let egg_time = mob.entity.random.next_int_bounded(6000) + 6000;
            let MobKind::Chicken(chicken) = &mut mob.kind;
            chicken.egg_time = egg_time;
        }
    }

    pub fn select_chicken_variant_to_spawn(&mut self, pos: BlockPos) -> Option<i32> {
        let biome = self.get_biome(pos).0 as i32;
        let mut entries: Vec<(i32, &SpawnSelector)> = spawn_selectors().iter().map(|(id, s)| (*id, s)).collect();
        entries.sort_by_key(|(_, s)| std::cmp::Reverse(s.priority));
        let mut highest_matched_priority = i32::MIN;
        entries.retain(|(_, s)| {
            if s.priority < highest_matched_priority {
                return false;
            }
            if s.biomes.as_ref().is_none_or(|biomes| biomes.contains(&biome)) {
                highest_matched_priority = s.priority;
                true
            } else {
                false
            }
        });
        if entries.is_empty() {
            return None;
        }
        let index = self.random.next_int_bounded(entries.len() as i32);
        Some(entries[index as usize].0)
    }

    pub fn pick_random_chicken_sound_variant(&mut self) -> i32 {
        self.random.next_int_bounded(sound_variants().entries.len() as i32)
    }
}
