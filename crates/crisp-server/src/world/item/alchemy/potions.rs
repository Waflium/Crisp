use crisp_data::MobEffect;

use crate::world::effect::MobEffectInstance;

pub struct Potion {
    pub id: &'static str,
    pub name: &'static str,
    effects: &'static [(&'static str, i32, i32)],
}

impl Potion {
    pub fn effects(&self) -> Vec<MobEffectInstance> {
        self.effects
            .iter()
            .map(|&(effect, duration, amplifier)| MobEffectInstance::new(MobEffect::by_name(effect).expect("vanilla effect"), duration, amplifier))
            .collect()
    }
}

const fn potion(id: &'static str, name: &'static str, effects: &'static [(&'static str, i32, i32)]) -> Potion {
    Potion { id, name, effects }
}

pub static POTIONS: &[Potion] = &[
    potion("minecraft:water", "water", &[]),
    potion("minecraft:mundane", "mundane", &[]),
    potion("minecraft:thick", "thick", &[]),
    potion("minecraft:awkward", "awkward", &[]),
    potion("minecraft:night_vision", "night_vision", &[("minecraft:night_vision", 3600, 0)]),
    potion("minecraft:long_night_vision", "night_vision", &[("minecraft:night_vision", 9600, 0)]),
    potion("minecraft:invisibility", "invisibility", &[("minecraft:invisibility", 3600, 0)]),
    potion("minecraft:long_invisibility", "invisibility", &[("minecraft:invisibility", 9600, 0)]),
    potion("minecraft:leaping", "leaping", &[("minecraft:jump_boost", 3600, 0)]),
    potion("minecraft:long_leaping", "leaping", &[("minecraft:jump_boost", 9600, 0)]),
    potion("minecraft:strong_leaping", "leaping", &[("minecraft:jump_boost", 1800, 1)]),
    potion("minecraft:fire_resistance", "fire_resistance", &[("minecraft:fire_resistance", 3600, 0)]),
    potion("minecraft:long_fire_resistance", "fire_resistance", &[("minecraft:fire_resistance", 9600, 0)]),
    potion("minecraft:swiftness", "swiftness", &[("minecraft:speed", 3600, 0)]),
    potion("minecraft:long_swiftness", "swiftness", &[("minecraft:speed", 9600, 0)]),
    potion("minecraft:strong_swiftness", "swiftness", &[("minecraft:speed", 1800, 1)]),
    potion("minecraft:slowness", "slowness", &[("minecraft:slowness", 1800, 0)]),
    potion("minecraft:long_slowness", "slowness", &[("minecraft:slowness", 4800, 0)]),
    potion("minecraft:strong_slowness", "slowness", &[("minecraft:slowness", 400, 3)]),
    potion("minecraft:turtle_master", "turtle_master", &[("minecraft:slowness", 400, 3), ("minecraft:resistance", 400, 2)]),
    potion("minecraft:long_turtle_master", "turtle_master", &[("minecraft:slowness", 800, 3), ("minecraft:resistance", 800, 2)]),
    potion("minecraft:strong_turtle_master", "turtle_master", &[("minecraft:slowness", 400, 5), ("minecraft:resistance", 400, 3)]),
    potion("minecraft:water_breathing", "water_breathing", &[("minecraft:water_breathing", 3600, 0)]),
    potion("minecraft:long_water_breathing", "water_breathing", &[("minecraft:water_breathing", 9600, 0)]),
    potion("minecraft:healing", "healing", &[("minecraft:instant_health", 1, 0)]),
    potion("minecraft:strong_healing", "healing", &[("minecraft:instant_health", 1, 1)]),
    potion("minecraft:harming", "harming", &[("minecraft:instant_damage", 1, 0)]),
    potion("minecraft:strong_harming", "harming", &[("minecraft:instant_damage", 1, 1)]),
    potion("minecraft:poison", "poison", &[("minecraft:poison", 900, 0)]),
    potion("minecraft:long_poison", "poison", &[("minecraft:poison", 1800, 0)]),
    potion("minecraft:strong_poison", "poison", &[("minecraft:poison", 432, 1)]),
    potion("minecraft:regeneration", "regeneration", &[("minecraft:regeneration", 900, 0)]),
    potion("minecraft:long_regeneration", "regeneration", &[("minecraft:regeneration", 1800, 0)]),
    potion("minecraft:strong_regeneration", "regeneration", &[("minecraft:regeneration", 450, 1)]),
    potion("minecraft:strength", "strength", &[("minecraft:strength", 3600, 0)]),
    potion("minecraft:long_strength", "strength", &[("minecraft:strength", 9600, 0)]),
    potion("minecraft:strong_strength", "strength", &[("minecraft:strength", 1800, 1)]),
    potion("minecraft:weakness", "weakness", &[("minecraft:weakness", 1800, 0)]),
    potion("minecraft:long_weakness", "weakness", &[("minecraft:weakness", 4800, 0)]),
    potion("minecraft:luck", "luck", &[("minecraft:luck", 6000, 0)]),
    potion("minecraft:slow_falling", "slow_falling", &[("minecraft:slow_falling", 1800, 0)]),
    potion("minecraft:long_slow_falling", "slow_falling", &[("minecraft:slow_falling", 4800, 0)]),
    potion("minecraft:wind_charged", "wind_charged", &[("minecraft:wind_charged", 3600, 0)]),
    potion("minecraft:weaving", "weaving", &[("minecraft:weaving", 3600, 0)]),
    potion("minecraft:oozing", "oozing", &[("minecraft:oozing", 3600, 0)]),
    potion("minecraft:infested", "infested", &[("minecraft:infested", 3600, 0)]),
];

pub fn by_id(id: &str) -> Option<&'static Potion> {
    POTIONS.iter().find(|p| p.id == id)
}
