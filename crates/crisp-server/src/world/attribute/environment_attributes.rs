#![allow(dead_code)]

use std::sync::Arc;

use super::environment_attribute::EnvironmentAttributeInfo as A;
use super::{AmbientParticle, AttributeRange, AttributeType as T, BedRule, EnvironmentAttribute, Value};
use crate::util::argb::{self, Vec3f, Vec4f};
use crate::util::tri_state::TriState;
use crate::world::entity::schedule::Activity;
use crate::world::level::biome::mob_spawn_settings::MobSpawnSettings;
use crate::world::level::moon_phase::MoonPhase;

pub const FOG_COLOR: EnvironmentAttribute<Vec3f> = EnvironmentAttribute::new(0);
pub const FOG_START_DISTANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(1);
pub const FOG_END_DISTANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(2);
pub const SKY_FOG_END_DISTANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(3);
pub const CLOUD_FOG_END_DISTANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(4);
pub const WATER_FOG_COLOR: EnvironmentAttribute<Vec3f> = EnvironmentAttribute::new(5);
pub const WATER_FOG_START_DISTANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(6);
pub const WATER_FOG_END_DISTANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(7);
pub const SKY_COLOR: EnvironmentAttribute<Vec3f> = EnvironmentAttribute::new(8);
pub const SUNRISE_SUNSET_COLOR: EnvironmentAttribute<Vec4f> = EnvironmentAttribute::new(9);
pub const CLOUD_COLOR: EnvironmentAttribute<Vec4f> = EnvironmentAttribute::new(10);
pub const CLOUD_HEIGHT: EnvironmentAttribute<f32> = EnvironmentAttribute::new(11);
pub const SUN_ANGLE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(12);
pub const MOON_ANGLE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(13);
pub const STAR_ANGLE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(14);
pub const MOON_PHASE: EnvironmentAttribute<MoonPhase> = EnvironmentAttribute::new(15);
pub const STAR_BRIGHTNESS: EnvironmentAttribute<f32> = EnvironmentAttribute::new(16);
pub const BLOCK_LIGHT_TINT: EnvironmentAttribute<Vec3f> = EnvironmentAttribute::new(17);
pub const SKY_LIGHT_COLOR: EnvironmentAttribute<Vec3f> = EnvironmentAttribute::new(18);
pub const SKY_LIGHT_FACTOR: EnvironmentAttribute<f32> = EnvironmentAttribute::new(19);
pub const NIGHT_VISION_COLOR: EnvironmentAttribute<Vec3f> = EnvironmentAttribute::new(20);
pub const AMBIENT_LIGHT_COLOR: EnvironmentAttribute<Vec3f> = EnvironmentAttribute::new(21);
pub const DEFAULT_DRIPSTONE_PARTICLE: EnvironmentAttribute<Arc<serde_json::Value>> = EnvironmentAttribute::new(22);
pub const AMBIENT_PARTICLES: EnvironmentAttribute<Arc<Vec<AmbientParticle>>> = EnvironmentAttribute::new(23);
pub const BACKGROUND_MUSIC: EnvironmentAttribute<Arc<serde_json::Value>> = EnvironmentAttribute::new(24);
pub const MUSIC_VOLUME: EnvironmentAttribute<f32> = EnvironmentAttribute::new(25);
pub const AMBIENT_SOUNDS: EnvironmentAttribute<Arc<serde_json::Value>> = EnvironmentAttribute::new(26);
pub const FIREFLY_BUSH_SOUNDS: EnvironmentAttribute<bool> = EnvironmentAttribute::new(27);
pub const SKY_LIGHT_LEVEL: EnvironmentAttribute<f32> = EnvironmentAttribute::new(28);
pub const CAN_START_RAID: EnvironmentAttribute<bool> = EnvironmentAttribute::new(29);
pub const WATER_EVAPORATES: EnvironmentAttribute<bool> = EnvironmentAttribute::new(30);
pub const BED_RULE: EnvironmentAttribute<Arc<BedRule>> = EnvironmentAttribute::new(31);
pub const STRAW_BED_RULE: EnvironmentAttribute<Arc<BedRule>> = EnvironmentAttribute::new(32);
pub const RESPAWN_ANCHOR_WORKS: EnvironmentAttribute<bool> = EnvironmentAttribute::new(33);
pub const NETHER_PORTAL_SPAWNS_PIGLINS: EnvironmentAttribute<bool> = EnvironmentAttribute::new(34);
pub const FAST_LAVA: EnvironmentAttribute<bool> = EnvironmentAttribute::new(35);
pub const INCREASED_FIRE_BURNOUT: EnvironmentAttribute<bool> = EnvironmentAttribute::new(36);
pub const EYEBLOSSOM_OPEN: EnvironmentAttribute<TriState> = EnvironmentAttribute::new(37);
pub const TURTLE_EGG_HATCH_CHANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(38);
pub const PIGLINS_ZOMBIFY: EnvironmentAttribute<bool> = EnvironmentAttribute::new(39);
pub const SNOW_GOLEM_MELTS: EnvironmentAttribute<bool> = EnvironmentAttribute::new(40);
pub const CREAKING_ACTIVE: EnvironmentAttribute<bool> = EnvironmentAttribute::new(41);
pub const SURFACE_SLIME_SPAWN_CHANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(42);
pub const CAT_WAKING_UP_GIFT_CHANCE: EnvironmentAttribute<f32> = EnvironmentAttribute::new(43);
pub const BEES_STAY_IN_HIVE: EnvironmentAttribute<bool> = EnvironmentAttribute::new(44);
pub const MONSTERS_BURN: EnvironmentAttribute<bool> = EnvironmentAttribute::new(45);
pub const CAN_PILLAGER_PATROL_SPAWN: EnvironmentAttribute<bool> = EnvironmentAttribute::new(46);
pub const NATURAL_MOB_SPAWNS: EnvironmentAttribute<Arc<MobSpawnSettings>> = EnvironmentAttribute::new(47);
pub const CREATURE_WORLD_GEN_SPAWN_PROBABILITY: EnvironmentAttribute<f32> = EnvironmentAttribute::new(48);
pub const VILLAGER_ACTIVITY: EnvironmentAttribute<Activity> = EnvironmentAttribute::new(49);
pub const BABY_VILLAGER_ACTIVITY: EnvironmentAttribute<Activity> = EnvironmentAttribute::new(50);

fn rgb(color: i32) -> Value {
    Value::Rgb(argb::vector3f_from_rgb24(color))
}

fn idle() -> Value {
    Value::Activity(Activity::by_name("minecraft:idle").expect("idle activity"))
}

fn empty_data() -> Value {
    Value::Data(Arc::new(serde_json::json!({})))
}

pub static ATTRIBUTES: [A; 51] = [
    A::builder(0, "minecraft:visual/fog_color", T::RgbColor, || rgb(0)).spatially_interpolated().syncable(),
    A::builder(1, "minecraft:visual/fog_start_distance", T::Float, || Value::Float(0.0)).spatially_interpolated().syncable(),
    A::builder(2, "minecraft:visual/fog_end_distance", T::Float, || Value::Float(1024.0))
        .value_range(AttributeRange::NON_NEGATIVE_FLOAT)
        .spatially_interpolated()
        .syncable(),
    A::builder(3, "minecraft:visual/sky_fog_end_distance", T::Float, || Value::Float(512.0))
        .value_range(AttributeRange::NON_NEGATIVE_FLOAT)
        .spatially_interpolated()
        .syncable(),
    A::builder(4, "minecraft:visual/cloud_fog_end_distance", T::Float, || Value::Float(2048.0))
        .value_range(AttributeRange::NON_NEGATIVE_FLOAT)
        .spatially_interpolated()
        .syncable(),
    A::builder(5, "minecraft:visual/water_fog_color", T::RgbColor, || rgb(-16448205)).spatially_interpolated().syncable(),
    A::builder(6, "minecraft:visual/water_fog_start_distance", T::Float, || Value::Float(-8.0)).spatially_interpolated().syncable(),
    A::builder(7, "minecraft:visual/water_fog_end_distance", T::Float, || Value::Float(96.0))
        .value_range(AttributeRange::NON_NEGATIVE_FLOAT)
        .spatially_interpolated()
        .syncable(),
    A::builder(8, "minecraft:visual/sky_color", T::RgbColor, || rgb(0)).spatially_interpolated().syncable(),
    A::builder(9, "minecraft:visual/sunrise_sunset_color", T::ArgbColor, || Value::Argb(argb::vector4f_from_argb32(0))).spatially_interpolated().syncable(),
    A::builder(10, "minecraft:visual/cloud_color", T::ArgbColor, || Value::Argb(argb::vector4f_from_argb32(0))).spatially_interpolated().syncable(),
    A::builder(11, "minecraft:visual/cloud_height", T::Float, || Value::Float(192.33)).spatially_interpolated().syncable(),
    A::builder(12, "minecraft:visual/sun_angle", T::AngleDegrees, || Value::Float(0.0)).spatially_interpolated().syncable(),
    A::builder(13, "minecraft:visual/moon_angle", T::AngleDegrees, || Value::Float(0.0)).spatially_interpolated().syncable(),
    A::builder(14, "minecraft:visual/star_angle", T::AngleDegrees, || Value::Float(0.0)).spatially_interpolated().syncable(),
    A::builder(15, "minecraft:visual/moon_phase", T::MoonPhase, || Value::MoonPhase(MoonPhase::FullMoon)).syncable(),
    A::builder(16, "minecraft:visual/star_brightness", T::Float, || Value::Float(0.0))
        .value_range(AttributeRange::UNIT_FLOAT)
        .spatially_interpolated()
        .syncable(),
    A::builder(17, "minecraft:visual/block_light_tint", T::RgbColor, || rgb(-10100)).spatially_interpolated().syncable(),
    A::builder(18, "minecraft:visual/sky_light_color", T::RgbColor, || rgb(-1)).spatially_interpolated().syncable(),
    A::builder(19, "minecraft:visual/sky_light_factor", T::Float, || Value::Float(1.0))
        .value_range(AttributeRange::UNIT_FLOAT)
        .spatially_interpolated()
        .syncable(),
    A::builder(20, "minecraft:visual/night_vision_color", T::RgbColor, || rgb(-6710887)).spatially_interpolated().syncable(),
    A::builder(21, "minecraft:visual/ambient_light_color", T::RgbColor, || rgb(-16777216)).spatially_interpolated().syncable(),
    A::builder(22, "minecraft:visual/default_dripstone_particle", T::Particle, || {
        Value::Data(Arc::new(serde_json::json!({ "type": "minecraft:dripping_dripstone_water" })))
    })
    .syncable(),
    A::builder(23, "minecraft:visual/ambient_particles", T::AmbientParticles, || Value::AmbientParticles(Arc::new(Vec::new()))).syncable(),
    A::builder(24, "minecraft:audio/background_music", T::BackgroundMusic, empty_data).syncable(),
    A::builder(25, "minecraft:audio/music_volume", T::Float, || Value::Float(1.0)).value_range(AttributeRange::UNIT_FLOAT).syncable(),
    A::builder(26, "minecraft:audio/ambient_sounds", T::AmbientSounds, empty_data).syncable(),
    A::builder(27, "minecraft:audio/firefly_bush_sounds", T::Boolean, || Value::Boolean(false)).syncable(),
    A::builder(28, "minecraft:gameplay/sky_light_level", T::Float, || Value::Float(15.0))
        .value_range(AttributeRange::Float(0.0, 15.0))
        .not_positional()
        .syncable(),
    A::builder(29, "minecraft:gameplay/can_start_raid", T::Boolean, || Value::Boolean(true)),
    A::builder(30, "minecraft:gameplay/water_evaporates", T::Boolean, || Value::Boolean(false)).syncable(),
    A::builder(31, "minecraft:gameplay/bed_rule", T::BedRule, || Value::BedRule(Arc::new(BedRule::can_sleep_when_dark()))),
    A::builder(32, "minecraft:gameplay/straw_bed_rule", T::BedRule, || Value::BedRule(Arc::new(BedRule::destroy_on_leave()))),
    A::builder(33, "minecraft:gameplay/respawn_anchor_works", T::Boolean, || Value::Boolean(false)),
    A::builder(34, "minecraft:gameplay/nether_portal_spawns_piglin", T::Boolean, || Value::Boolean(false)),
    A::builder(35, "minecraft:gameplay/fast_lava", T::Boolean, || Value::Boolean(false)).not_positional().syncable(),
    A::builder(36, "minecraft:gameplay/increased_fire_burnout", T::Boolean, || Value::Boolean(false)),
    A::builder(37, "minecraft:gameplay/eyeblossom_open", T::TriState, || Value::TriState(TriState::Default)),
    A::builder(38, "minecraft:gameplay/turtle_egg_hatch_chance", T::Float, || Value::Float(0.002)).value_range(AttributeRange::UNIT_FLOAT),
    A::builder(39, "minecraft:gameplay/piglins_zombify", T::Boolean, || Value::Boolean(true)).syncable(),
    A::builder(40, "minecraft:gameplay/snow_golem_melts", T::Boolean, || Value::Boolean(false)),
    A::builder(41, "minecraft:gameplay/creaking_active", T::Boolean, || Value::Boolean(false)).syncable(),
    A::builder(42, "minecraft:gameplay/surface_slime_spawn_chance", T::Float, || Value::Float(0.0)).value_range(AttributeRange::UNIT_FLOAT),
    A::builder(43, "minecraft:gameplay/cat_waking_up_gift_chance", T::Float, || Value::Float(0.0)).value_range(AttributeRange::UNIT_FLOAT),
    A::builder(44, "minecraft:gameplay/bees_stay_in_hive", T::Boolean, || Value::Boolean(false)),
    A::builder(45, "minecraft:gameplay/monsters_burn", T::Boolean, || Value::Boolean(false)),
    A::builder(46, "minecraft:gameplay/can_pillager_patrol_spawn", T::Boolean, || Value::Boolean(true)),
    A::builder(47, "minecraft:gameplay/natural_mob_spawns", T::MobSpawnSettings, || Value::MobSpawnSettings(Arc::new(MobSpawnSettings::default())))
        .full_resolution_biomes(),
    A::builder(48, "minecraft:gameplay/creature_world_gen_spawn_probability", T::Float, || Value::Float(0.1))
        .value_range(AttributeRange::UNIT_FLOAT_EPSILON),
    A::builder(49, "minecraft:gameplay/villager_activity", T::Activity, idle),
    A::builder(50, "minecraft:gameplay/baby_villager_activity", T::Activity, idle),
];

#[cfg(test)]
mod tests {
    use super::ATTRIBUTES;

    #[test]
    fn registry_order_matches_vanilla() {
        let names: Vec<&str> = ATTRIBUTES.iter().map(|a| a.name).collect();
        assert_eq!(names, crisp_data::registry::ENVIRONMENT_ATTRIBUTE);
        assert!(ATTRIBUTES.iter().enumerate().all(|(i, a)| a.id == i));
        let types: Vec<&str> = super::super::AttributeType::ALL.iter().map(|t| t.name()).collect();
        assert_eq!(types, crisp_data::registry::ATTRIBUTE_TYPE);
    }
}
