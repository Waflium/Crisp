use std::collections::HashMap;
use std::sync::Arc;

use crisp_core::{BlockPos, Vec3, mth};

use super::environment_attribute_system::EnvironmentAccess;
use super::environment_attributes::{self as attributes, ATTRIBUTES};
use super::modifier::{AttributeModifier, BlendToGray, BooleanModifier, ColorModifier, FloatModifier, FloatWithAlpha, IntegerModifier};
use super::{AttributeType, AttributeValue, EnvironmentAttribute, EnvironmentAttributeSystem, LerpFunction, Value, bed_rule};
use crate::server::level::ServerLevel;
use crate::server::level::chunk_cache::ChunkCache;
use crate::util::argb;
use crate::util::easing_type::{CubicBezier, EasingType};
use crate::util::tri_state::TriState;
use crate::world::clock::ServerClockManager;
use crate::world::level::biome::BIOME_ATTRIBUTES;
use crate::world::level::chunk::{Biome, ChunkGenerator};
use crate::world::level::levelgen::chunk_generator::NoiseBasedChunkGenerator;

const ORACLE: &str = include_str!("oracle.txt");

impl AttributeValue for Value {
    fn from_value(value: &Value) -> Self {
        value.clone()
    }
}

fn f(v: f32) -> String {
    format!("f:{:x}", v.to_bits())
}

fn d(v: f64) -> String {
    format!("{:x}", v.to_bits())
}

fn fmt(value: &Value) -> String {
    match value {
        Value::Boolean(b) => b.to_string(),
        Value::Float(v) => f(*v),
        Value::Integer(i) => format!("i:{i}"),
        Value::Rgb(c) => format!("rgb:{},{},{}", f(c[0]), f(c[1]), f(c[2])),
        Value::Argb(c) => format!("argb:{},{},{},{}", f(c[0]), f(c[1]), f(c[2]), f(c[3])),
        Value::MoonPhase(p) => p.name().into(),
        Value::TriState(t) => match t {
            TriState::True => "true",
            TriState::False => "false",
            TriState::Default => "default",
        }
        .into(),
        Value::Activity(a) => a.name().into(),
        Value::BedRule(r) => {
            let rule = |r: bed_rule::Rule| match r {
                bed_rule::Rule::Always => "always",
                bed_rule::Rule::WhenDark => "when_dark",
                bed_rule::Rule::Never => "never",
            };
            format!("bed:{},{},{},{},{}", rule(r.can_sleep), rule(r.can_set_spawn), r.destroy_on_use, r.destroy_on_leave, r.error_message.is_some())
        }
        Value::MobSpawnSettings(s) => {
            let mut out = String::from("mobs:");
            for category in crate::world::level::biome::mob_spawn_settings::MobCategory::VALUES {
                if let Some(list) = s.mobs_in_category(category) {
                    out += &format!("{}=", category.name());
                    for (data, weight) in list {
                        out += &format!("{}*{weight},", crisp_data::registry::ENTITY_TYPE[data.entity_type as usize]);
                    }
                    out.push(';');
                }
            }
            let mut costs: Vec<_> = s.mob_spawn_costs.iter().map(|(e, c)| (crisp_data::registry::ENTITY_TYPE[*e as usize], c)).collect();
            costs.sort_by_key(|c| c.0);
            for (entity, cost) in costs {
                out += &format!("cost={entity}:{}:{};", d(cost.energy_budget), d(cost.charge));
            }
            out
        }
        Value::AmbientParticles(list) => format!("particles:{}", list.iter().map(|p| f(p.probability) + ",").collect::<String>()),
        _ => "data".into(),
    }
}

fn java_hash(s: &str) -> String {
    format!("{:x}", s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32)) as u32)
}

fn oracle(kind: &str) -> HashMap<String, String> {
    ORACLE
        .lines()
        .filter_map(|l| l.strip_prefix(kind)?.strip_prefix(' '))
        .map(|l| {
            let (key, value) = l.rsplit_once(' ').unwrap();
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn path(id: usize) -> &'static str {
    ATTRIBUTES[id].name.strip_prefix("minecraft:").unwrap()
}

#[test]
fn definitions_match_vanilla() {
    let expected = oracle("attr");
    assert_eq!(expected.len(), ATTRIBUTES.len());
    for a in &ATTRIBUTES {
        let line = format!(
            "{} {} {} {} {} {}",
            a.ty.name(),
            a.is_syncable,
            a.is_positional,
            a.is_spatially_interpolated,
            a.full_resolution_biomes,
            fmt(a.default_value())
        );
        let (type_and_flags, default) = line.rsplit_once(' ').unwrap();
        let key = format!("{} {type_and_flags}", path(a.id));
        assert_eq!(expected.get(&key).map(String::as_str), Some(default), "{}", a.name);
    }
}

#[test]
fn modifiers_and_lerps_match_vanilla() {
    let expected = oracle("mod");
    let mut checked = 0;
    let mut check = |label: String, value: Value| {
        assert_eq!(expected.get(&label), Some(&fmt(&value)), "{label}");
        checked += 1;
    };
    let fs = [0.3f32, -2.5, 15.0];
    let fa = [0.75f32, 4.0, -0.0];
    let ops = ["add", "subtract", "multiply", "minimum", "maximum"];
    let float_mods = [FloatModifier::Add, FloatModifier::Subtract, FloatModifier::Multiply, FloatModifier::Minimum, FloatModifier::Maximum];
    for (o, m) in float_mods.iter().enumerate() {
        for (i, s) in fs.iter().enumerate() {
            for (j, a) in fa.iter().enumerate() {
                check(format!("float {} {i} {j}", ops[o]), AttributeModifier::Float(*m).apply(&Value::Float(*s), &Value::Float(*a)));
            }
        }
    }
    let alphas = [FloatWithAlpha { value: 4.0, alpha: 0.3125 }, FloatWithAlpha { value: 0.24, alpha: 0.52734375 }, FloatWithAlpha { value: 7.0, alpha: 1.0 }];
    for (i, s) in fs.iter().enumerate() {
        for (j, a) in alphas.iter().enumerate() {
            check(format!("float alpha_blend {i} {j}"), FloatModifier::AlphaBlend.apply(*s, &Value::FloatWithAlpha(*a)));
        }
    }
    let bools = [
        ("and", BooleanModifier::And),
        ("nand", BooleanModifier::Nand),
        ("or", BooleanModifier::Or),
        ("nor", BooleanModifier::Nor),
        ("xor", BooleanModifier::Xor),
        ("xnor", BooleanModifier::Xnor),
    ];
    for (name, m) in bools {
        for s in [false, true] {
            for a in [false, true] {
                check(format!("bool {name} {s} {a}"), Value::Boolean(m.apply(s, a)));
            }
        }
    }
    let is = [7, -3, i32::MAX];
    let ia = [2, -5];
    let int_mods = [IntegerModifier::Add, IntegerModifier::Subtract, IntegerModifier::Multiply, IntegerModifier::Minimum, IntegerModifier::Maximum];
    for (o, m) in int_mods.iter().enumerate() {
        for (i, s) in is.iter().enumerate() {
            for (j, a) in ia.iter().enumerate() {
                check(format!("int {} {i} {j}", ops[o]), Value::Integer(m.apply(*s, *a)));
            }
        }
    }
    let night_sky_light = argb::color_from_float(1.0, 0.48, 0.48, 1.0);
    let rgb_subjects = [0x78a7ff, 0xc0d8ff, 0x000000];
    let argb_args = [argb::color_with_alpha(0.3125, night_sky_light), 0x80ff0000u32 as i32, 0x00123456, 0xff00ff00u32 as i32, 0x40404040];
    let rgb_args = [0x7a7aff, 0xffffff, 0x0f0f0f];
    let grays = [(0.6, 0.75), (0.24, 0.94), (0.095, 0.94)];
    let argb_subjects = [0xccffffffu32 as i32, 0x5fefc033, 0x00000000, 0xffffffffu32 as i32];
    let rgb = |c: i32| Value::Rgb(argb::vector3f_from_rgb24(c));
    let argb = |c: i32| Value::Argb(argb::vector4f_from_argb32(c));
    let gray = |g: (f32, f32)| Value::BlendToGray(BlendToGray { brightness: g.0, factor: g.1 });
    for (i, &s) in rgb_subjects.iter().enumerate() {
        for (j, &a) in argb_args.iter().enumerate() {
            check(format!("rgb alpha_blend {i} {j}"), ColorModifier::AlphaBlendRgb.apply(&rgb(s), &argb(a)));
        }
        for (j, &a) in rgb_args.iter().enumerate() {
            check(format!("rgb add {i} {j}"), ColorModifier::AddRgb.apply(&rgb(s), &rgb(a)));
            check(format!("rgb subtract {i} {j}"), ColorModifier::SubtractRgb.apply(&rgb(s), &rgb(a)));
            check(format!("rgb multiply {i} {j}"), ColorModifier::MultiplyRgb.apply(&rgb(s), &rgb(a)));
        }
        for (j, &g) in grays.iter().enumerate() {
            check(format!("rgb blend_to_gray {i} {j}"), ColorModifier::BlendToGrayRgb.apply(&rgb(s), &gray(g)));
        }
    }
    for (i, &s) in argb_subjects.iter().enumerate() {
        for (j, &a) in argb_args.iter().enumerate() {
            check(format!("argb alpha_blend {i} {j}"), ColorModifier::AlphaBlendArgb.apply(&argb(s), &argb(a)));
            check(format!("argb multiply {i} {j}"), ColorModifier::MultiplyArgb.apply(&argb(s), &argb(a)));
        }
        for (j, &a) in rgb_args.iter().enumerate() {
            check(format!("argb add {i} {j}"), ColorModifier::AddArgb.apply(&argb(s), &rgb(a)));
            check(format!("argb subtract {i} {j}"), ColorModifier::SubtractArgb.apply(&argb(s), &rgb(a)));
        }
        for (j, &g) in grays.iter().enumerate() {
            check(format!("argb blend_to_gray {i} {j}"), ColorModifier::BlendToGrayArgb.apply(&argb(s), &gray(g)));
        }
    }
    let degrees = [(0.25, 350.0, 10.0), (0.5, 0.0, 170.0), (0.5, 10.0, 200.0), (0.75, -90.0, 45.0), (0.3, 720.0, 30.0)];
    for (i, &(alpha, from, to)) in degrees.iter().enumerate() {
        check(format!("lerp degrees {i}"), AttributeType::AngleDegrees.partial_tick_lerp().apply(alpha, &Value::Float(from), &Value::Float(to)));
        check(format!("lerp float {i}"), AttributeType::Float.keyframe_lerp().apply(alpha, &Value::Float(from), &Value::Float(to)));
    }
    for (i, &(alpha, from, to)) in [(0.5, 3, 10), (0.3, 10, -7), (0.99, 0, 1)].iter().enumerate() {
        check(format!("lerp int {i}"), AttributeType::Integer.keyframe_lerp().apply(alpha, &Value::Integer(from), &Value::Integer(to)));
    }
    check("lerp rgb".into(), LerpFunction::ColorVec3.apply(0.37, &rgb(0x78a7ff), &rgb(0x302821)));
    check("lerp argb".into(), LerpFunction::ColorVec4.apply(0.61, &argb(0xccffffffu32 as i32), &argb(0x5fefc033)));
    for (label, alpha) in [("0.0", 0.0), ("0.49", 0.49), ("0.5", 0.5), ("0.99", 0.99), ("1.0", 1.0)] {
        let (from, to) = (Value::Boolean(false), Value::Boolean(true));
        check(format!("lerp keyframe {label}"), AttributeType::Boolean.keyframe_lerp().apply(alpha, &from, &to));
        check(format!("lerp state_change {label}"), AttributeType::Boolean.state_change_lerp().apply(alpha, &from, &to));
        check(format!("lerp spatial {label}"), AttributeType::Boolean.spatial_lerp().apply(alpha, &from, &to));
    }
    let biomes = ["plains", "desert", "mushroom_fields", "deep_dark", "soul_sand_valley", "the_void", "warm_ocean"];
    let spawns = |name: &str| {
        let biome = Biome::by_name(&format!("minecraft:{name}")).unwrap();
        let empty = ATTRIBUTES[attributes::NATURAL_MOB_SPAWNS.id()].default_value();
        BIOME_ATTRIBUTES[biome.0 as usize].get(attributes::NATURAL_MOB_SPAWNS.id()).map_or(empty.clone(), |e| e.apply_modifier(empty))
    };
    for a in biomes {
        for b in biomes {
            check(format!("overlay {a} {b}"), AttributeModifier::Overlay.apply(&spawns(a), &spawns(b)));
        }
    }
    assert_eq!(checked, expected.len());
}

#[test]
fn easing_matches_vanilla() {
    let expected = oracle("ease");
    let xs = [0.0f32, 0.1, 0.25, 0.5, 0.73, 0.9, 1.0];
    let mut names: Vec<&str> = expected.keys().map(|k| k.split(' ').next().unwrap()).filter(|n| *n != "bezier").collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 32);
    for name in names {
        let easing = EasingType::by_name(name).unwrap();
        for x in xs {
            assert_eq!(expected.get(&format!("{name} {}", f(x))), Some(&f(easing.apply(x))), "{name} at {x}");
        }
    }
    let bezier = CubicBezier::new(0.362, 0.241, 1.0 - 0.362, 1.0 - 0.241);
    for x in xs {
        assert_eq!(expected.get(&format!("bezier {}", f(x))), Some(&f(bezier.apply(x))), "bezier at {x}");
    }
}

struct Fixed {
    ticks: i64,
    rain: f32,
    thunder: f32,
}

impl EnvironmentAccess for Fixed {
    fn biome(&self, _: i32, _: i32, _: i32) -> Biome {
        unreachable!("dimension values never read biomes")
    }

    fn noise_biome(&self, _: i32, _: i32, _: i32) -> Biome {
        unreachable!("dimension values never read biomes")
    }

    fn clock_total_ticks(&self, _: usize) -> i64 {
        self.ticks
    }

    fn rain_level(&self) -> f32 {
        self.rain
    }

    fn thunder_level(&self) -> f32 {
        self.thunder
    }
}

fn dimension_type(name: &str) -> &'static crisp_data::DimensionType {
    crisp_data::DIMENSION_TYPES.iter().find(|d| d.name == name).unwrap()
}

#[test]
fn systems_follow_the_day_and_weather_like_vanilla() {
    let expected = oracle("dim");
    let times = [0, 133, 1000, 6000, 12000, 12600, 13000, 13670, 18000, 23460, 23999, 30000, 100000, 191000];
    let mut matched = 0;
    for (dimension, can_have_weather) in [("overworld", true), ("the_nether", false), ("the_end", false)] {
        let mut system = EnvironmentAttributeSystem::builder().add_default_layers(dimension_type(&format!("minecraft:{dimension}")), can_have_weather).build();
        let weathers: &[(f32, f32)] = if can_have_weather { &[(0.0, 0.0), (0.4, 0.0), (1.0, 0.6)] } else { &[(0.0, 0.0)] };
        for ticks in times {
            for &(rain, thunder) in weathers {
                let access = Fixed { ticks, rain, thunder };
                system.invalidate_tick_cache();
                for a in &ATTRIBUTES {
                    let value = system.get_dimension_value(EnvironmentAttribute::<Value>::new(a.id), &access);
                    let key = format!("{dimension} {ticks} {} {} {}", f(rain), f(thunder), path(a.id));
                    let want = expected.get(&key).cloned();
                    matched += usize::from(want.is_some());
                    assert_eq!(fmt(&value), want.unwrap_or_else(|| fmt(a.default_value())), "{key}");
                }
            }
        }
    }
    assert_eq!(matched, expected.len());
}

#[test]
fn constant_layers_fold_into_the_base_value() {
    let nether = EnvironmentAttributeSystem::builder().add_default_layers(dimension_type("minecraft:the_nether"), false).build();
    assert_eq!(nether.constant_base_value(attributes::FAST_LAVA.id()), &Value::Boolean(true));
    assert_eq!(nether.constant_base_value(attributes::SKY_LIGHT_LEVEL.id()), &Value::Float(4.0));
    assert!(!nether.is_affected_by_position(attributes::SKY_LIGHT_LEVEL.id()));
    assert!(nether.is_affected_by_position(attributes::NATURAL_MOB_SPAWNS.id()));
    assert!(!nether.is_affected_by_position(attributes::EYEBLOSSOM_OPEN.id()));
    assert_eq!(nether.constant_base_value(attributes::EYEBLOSSOM_OPEN.id()), &Value::TriState(TriState::Default));
}

#[test]
fn biome_values_match_vanilla_in_a_noise_world() {
    const SEED: i64 = 12345;
    let overworld = dimension_type("minecraft:overworld");
    let chunks = ChunkCache::new(overworld, ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::overworld(SEED))), None);
    let mut level = ServerLevel::new(crate::server::OVERWORLD, 0, chunks, None, 10, 10);
    level.biome_zoom_seed = crate::world::level::biome::obfuscate_seed(SEED);
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), 13000);
    level.sync_clocks(&clocks);

    let biome_provided: Vec<usize> = (0..ATTRIBUTES.len()).filter(|&id| BIOME_ATTRIBUTES.iter().any(|b| b.contains(id))).collect();
    let mut positions = 0;
    for line in ORACLE.lines().filter_map(|l| l.strip_prefix("pos ")) {
        let parts: Vec<&str> = line.split(' ').collect();
        let [x, y, z, attribute, value] = parts[..] else { panic!("{line}") };
        let pos = BlockPos::new(x.parse().unwrap(), y.parse().unwrap(), z.parse().unwrap());
        if attribute != "biome" {
            continue;
        }
        positions += 1;
        let center = Vec3::at_center_of(pos);
        let biome = level.get_noise_biome(mth::floor(center.x) >> 2, mth::floor(center.y) >> 2, mth::floor(center.z) >> 2);
        assert_eq!(biome.name(), format!("minecraft:{value}"), "biome at {pos:?}");
        for &id in &biome_provided {
            let got = fmt(&level.environment_attributes().get_value_at(EnvironmentAttribute::<Value>::new(id), pos));
            let got = if got.starts_with("mobs:") { format!("mobs#{}", java_hash(&got)) } else { got };
            let key = format!("{x} {y} {z} {}", path(id));
            let default = fmt(ATTRIBUTES[id].default_value());
            let want = ORACLE.lines().find_map(|l| l.strip_prefix("pos ")?.strip_prefix(&key)?.strip_prefix(' ')).map_or(default, str::to_owned);
            assert_eq!(got, want, "{key}");
        }
    }
    assert_eq!(positions, 108);
    assert!(level.increased_fire_burnout(BlockPos::new(-2592, 70, 2880)));
}

#[test]
fn sky_darkens_at_night_and_in_storms() {
    let mut level = crate::server::level::testing::level();
    let overworld_clock = ServerClockManager::id_of("minecraft:overworld").unwrap();
    let mut clocks = ServerClockManager::new();
    let expected = oracle("dim");
    for ticks in [0, 6000, 13000, 23460, 18000] {
        clocks.set_total_ticks(overworld_clock, ticks);
        level.sync_clocks(&clocks);
        for (rain, thunder) in [(0.0f32, 0.0f32), (0.4, 0.0), (1.0, 0.6)] {
            (level.rain_level, level.o_rain_level) = (rain, rain);
            let thunder_level = if rain > 0.0 { thunder / rain } else { 0.0 };
            (level.thunder_level, level.o_thunder_level) = (thunder_level, thunder_level);
            level.sync_clocks(&clocks);
            level.update_sky_brightness();
            let key = format!("overworld {ticks} {} {} gameplay/sky_light_level", f(rain), f(level.get_thunder_level(1.0)));
            let sky_light_level = expected.get(&key).map_or(15.0, |v| f32::from_bits(u32::from_str_radix(v.strip_prefix("f:").unwrap(), 16).unwrap()));
            assert_eq!(level.sky_darken(), (15.0 - sky_light_level) as i32, "{key}");
        }
    }
    assert!(level.is_dark_outside());
    assert_eq!(level.environment_attributes().get_value_at(attributes::MOON_PHASE, BlockPos::ZERO), crate::world::level::moon_phase::MoonPhase::FullMoon);
}
