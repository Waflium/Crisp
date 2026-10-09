use std::borrow::Cow;

use crisp_data::{MobEffect, mob_effects};

use super::MobEffectInstance;
use super::mob_effect::should_apply_effect_tick_this_tick;
use crate::commands::arguments::parse_identifier;
use crate::nbt::to_string;
use crate::world::entity::attributes::{Attribute, AttributeMap, AttributeModifier, Operation, PLAYER_ATTRIBUTES};

const ORACLE: &str = r#"nbt plain {duration:600,id:"minecraft:speed",show_icon:1b}
nbt amplified {amplifier:1b,duration:100,id:"minecraft:regeneration",show_icon:1b}
nbt infinite {ambient:1b,amplifier:-1b,duration:-1,id:"minecraft:night_vision",show_icon:0b,show_particles:0b}
nbt zero {id:"minecraft:luck",show_icon:0b}
update stronger true {amplifier:2b,duration:100,hidden_effect:{duration:400,show_icon:1b},id:"minecraft:strength",show_icon:1b}
update hidden false {amplifier:2b,duration:100,hidden_effect:{amplifier:1b,duration:200,hidden_effect:{duration:400,show_icon:1b},show_icon:1b},id:"minecraft:strength",show_icon:1b}
update weaker false {amplifier:2b,duration:100,hidden_effect:{amplifier:1b,duration:200,hidden_effect:{duration:400,show_icon:1b},show_icon:1b},id:"minecraft:strength",show_icon:1b}
update longer true {amplifier:2b,duration:300,hidden_effect:{amplifier:1b,duration:200,hidden_effect:{duration:400,show_icon:1b},show_icon:1b},id:"minecraft:strength",show_icon:1b,show_particles:0b}
ticked {amplifier:2b,duration:199,hidden_effect:{amplifier:1b,duration:99,hidden_effect:{duration:299,show_icon:1b},show_icon:1b},id:"minecraft:strength",show_icon:1b,show_particles:0b}
ticked {amplifier:2b,duration:99,hidden_effect:{amplifier:1b,hidden_effect:{duration:199,show_icon:1b},show_icon:1b},id:"minecraft:strength",show_icon:1b,show_particles:0b}
update infinite false {duration:-1,id:"minecraft:haste",show_icon:1b}
update over infinite true {amplifier:1b,duration:1000,hidden_effect:{duration:-1,show_icon:1b},id:"minecraft:haste",show_icon:1b}
apply minecraft:regeneration 0 3 [0, 50, 100]
apply minecraft:regeneration 1 5 [0, 25, 50, 75, 100]
apply minecraft:regeneration 2 11 [0, 12, 24, 36, 48, 60, 72, 84]
apply minecraft:regeneration 5 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:regeneration 6 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:regeneration 31 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:regeneration 32 3 [0, 50, 100]
apply minecraft:poison 0 5 [0, 25, 50, 75, 100]
apply minecraft:poison 1 11 [0, 12, 24, 36, 48, 60, 72, 84]
apply minecraft:poison 2 21 [0, 6, 12, 18, 24, 30, 36, 42]
apply minecraft:poison 5 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:poison 6 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:poison 31 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:poison 32 5 [0, 25, 50, 75, 100]
apply minecraft:wither 0 4 [0, 40, 80, 120]
apply minecraft:wither 1 7 [0, 20, 40, 60, 80, 100, 120]
apply minecraft:wither 2 13 [0, 10, 20, 30, 40, 50, 60, 70]
apply minecraft:wither 5 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:wither 6 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:wither 31 122 [-1, 0, 1, 2, 3, 4, 5, 6]
apply minecraft:wither 32 4 [0, 40, 80, 120]
apply minecraft:raid_omen 0 1 [1]
apply minecraft:raid_omen 1 1 [1]
apply minecraft:raid_omen 2 1 [1]
apply minecraft:raid_omen 5 1 [1]
apply minecraft:raid_omen 6 1 [1]
apply minecraft:raid_omen 31 1 [1]
apply minecraft:raid_omen 32 1 [1]
apply minecraft:instant_health 0 120 [1, 2, 3, 4, 5, 6, 7, 8]
apply minecraft:instant_health 1 120 [1, 2, 3, 4, 5, 6, 7, 8]
apply minecraft:instant_health 2 120 [1, 2, 3, 4, 5, 6, 7, 8]
apply minecraft:instant_health 5 120 [1, 2, 3, 4, 5, 6, 7, 8]
apply minecraft:instant_health 6 120 [1, 2, 3, 4, 5, 6, 7, 8]
apply minecraft:instant_health 31 120 [1, 2, 3, 4, 5, 6, 7, 8]
apply minecraft:instant_health 32 120 [1, 2, 3, 4, 5, 6, 7, 8]
apply minecraft:speed 0 0 []
apply minecraft:speed 1 0 []
apply minecraft:speed 2 0 []
apply minecraft:speed 5 0 []
apply minecraft:speed 6 0 []
apply minecraft:speed 31 0 []
apply minecraft:speed 32 0 []
attr speed 3fc2a3055718fc4f {base:0.10000000149011612d,id:"minecraft:movement_speed",modifiers:[{amount:0.6000000089406967d,id:"minecraft:effect.speed",operation:"add_multiplied_total"},{amount:-0.30000001192092896d,id:"minecraft:effect.slowness",operation:"add_multiplied_total"}]}
attr many 4000c36922f28c73 {base:1.0d,id:"minecraft:luck",modifiers:[{amount:0.001000000007d,id:"test:m1",operation:"add_multiplied_base"},{amount:0.002000000003d,id:"test:m2",operation:"add_multiplied_total"},{amount:0.004000000006d,id:"test:m4",operation:"add_multiplied_base"},{amount:0.005000000002d,id:"test:m5",operation:"add_multiplied_total"},{amount:0.0070000000050000006d,id:"test:m7",operation:"add_multiplied_base"},{amount:0.008000000001d,id:"test:m8",operation:"add_multiplied_total"},{amount:0.010000000004d,id:"test:m10",operation:"add_multiplied_base"},{amount:0.011d,id:"test:m11",operation:"add_multiplied_total"},{amount:0.013000000003d,id:"test:m13",operation:"add_multiplied_base"},{amount:0.014000000010000001d,id:"test:m14",operation:"add_multiplied_total"},{amount:0.016000000002d,id:"test:m16",operation:"add_multiplied_base"},{amount:0.017000000009d,id:"test:m17",operation:"add_multiplied_total"},{amount:0.019000000000999998d,id:"test:m19",operation:"add_multiplied_base"},{amount:0.020000000008d,id:"test:m20",operation:"add_multiplied_total"},{amount:0.022d,id:"test:m22",operation:"add_multiplied_base"},{amount:0.023000000007d,id:"test:m23",operation:"add_multiplied_total"},{amount:0.025000000010000002d,id:"test:m25",operation:"add_multiplied_base"},{amount:0.026000000006d,id:"test:m26",operation:"add_multiplied_total"},{amount:0.028000000009d,id:"test:m28",operation:"add_multiplied_base"},{amount:0.029000000005000002d,id:"test:m29",operation:"add_multiplied_total"},{amount:0.031000000008d,id:"test:m31",operation:"add_multiplied_base"},{amount:0.032000000004d,id:"test:m32",operation:"add_multiplied_total"},{amount:0.034000000007d,id:"test:m34",operation:"add_multiplied_base"},{amount:0.035000000003000006d,id:"test:m35",operation:"add_multiplied_total"},{amount:0.037000000006d,id:"test:m37",operation:"add_multiplied_base"},{amount:0.038000000001999996d,id:"test:m38",operation:"add_multiplied_total"},{amount:0.3d,id:"z:last",operation:"add_value"}]}
attr sums 3fb999999999999c {base:0.1d,id:"minecraft:armor",modifiers:[{amount:0.1d,id:"minecraft:a",operation:"add_value"},{amount:0.2d,id:"minecraft:b",operation:"add_value"},{amount:1.0E16d,id:"minecraft:c",operation:"add_value"},{amount:-1.0E16d,id:"minecraft:d",operation:"add_value"},{amount:0.3d,id:"minecraft:e",operation:"add_value"},{amount:3.0E-17d,id:"minecraft:f",operation:"add_value"}]}
attr clamped 4090000000000000 {base:20.0d,id:"minecraft:max_health",modifiers:[{amount:5000.0d,id:"minecraft:big",operation:"add_value"}]}"#;

fn oracle(prefix: &str) -> &'static str {
    ORACLE.lines().find_map(|l| l.strip_prefix(prefix)).unwrap_or_else(|| panic!("no oracle line {prefix}"))
}

fn nbt(effect: &MobEffectInstance) -> String {
    to_string(&effect.to_nbt())
}

#[test]
fn codec_matches_vanilla() {
    let cases = [
        ("plain", MobEffectInstance::new(mob_effects::SPEED, 600, 0)),
        ("amplified", MobEffectInstance::new(mob_effects::REGENERATION, 100, 1)),
        ("infinite", MobEffectInstance::with(mob_effects::NIGHT_VISION, -1, 255, true, false, false)),
        ("zero", MobEffectInstance::with(mob_effects::LUCK, 0, 0, false, true, false)),
    ];
    for (label, effect) in cases {
        assert_eq!(nbt(&effect), oracle(&format!("nbt {label} ")), "{label}");
        assert_eq!(MobEffectInstance::from_nbt(&effect.to_nbt()), Some(effect));
    }
}

#[test]
fn update_hides_and_downgrades_as_vanilla() {
    let mut stacked = MobEffectInstance::new(mob_effects::STRENGTH, 400, 0);
    let steps = [
        ("stronger", MobEffectInstance::new(mob_effects::STRENGTH, 100, 2)),
        ("hidden", MobEffectInstance::new(mob_effects::STRENGTH, 200, 1)),
        ("weaker", MobEffectInstance::with(mob_effects::STRENGTH, 50, 2, true, true, true)),
        ("longer", MobEffectInstance::with(mob_effects::STRENGTH, 300, 2, false, false, true)),
    ];
    for (label, take_over) in steps {
        let changed = stacked.update(&take_over);
        assert_eq!(format!("{changed} {}", nbt(&stacked)), oracle(&format!("update {label} ")), "{label}");
    }
    let mut ticked = ORACLE.lines().filter_map(|l| l.strip_prefix("ticked "));
    for ticks in [101, 100] {
        for _ in 0..ticks {
            if stacked.has_remaining_duration() {
                stacked.tick_down_duration();
                stacked.downgrade_to_hidden_effect();
            }
        }
        assert_eq!(nbt(&stacked), ticked.next().unwrap());
    }
    let mut infinite = MobEffectInstance::new(mob_effects::HASTE, -1, 0);
    let changed = infinite.update(&MobEffectInstance::new(mob_effects::HASTE, 1000, 0));
    assert_eq!(format!("{changed} {}", nbt(&infinite)), oracle("update infinite "));
    let changed = infinite.update(&MobEffectInstance::new(mob_effects::HASTE, 1000, 1));
    assert_eq!(format!("{changed} {}", nbt(&infinite)), oracle("update over infinite "));
}

#[test]
fn periodic_effects_apply_on_vanilla_ticks() {
    for line in ORACLE.lines().filter_map(|l| l.strip_prefix("apply ")) {
        let mut parts = line.splitn(3, ' ');
        let effect = MobEffect::by_name(parts.next().unwrap()).unwrap();
        let amplifier: i32 = parts.next().unwrap().parse().unwrap();
        let ticks: Vec<i32> = (-1..=120).filter(|&d| should_apply_effect_tick_this_tick(effect, d, amplifier)).collect();
        let shown: Vec<String> = ticks.iter().take(8).map(i32::to_string).collect();
        assert_eq!(format!("{} [{}]", ticks.len(), shown.join(", ")), parts.next().unwrap(), "{line}");
    }
}

fn attribute(label: &str, attribute: Attribute, base: f64, steps: &[(&str, &str, f64, u8)]) {
    let mut map = AttributeMap::new(&PLAYER_ATTRIBUTES);
    let instance = map.instance(attribute).unwrap();
    instance.set_base_value(base);
    for &(kind, id, amount, operation) in steps {
        let id = parse_identifier(id).unwrap();
        let modifier = AttributeModifier { id: Cow::Owned(id.clone()), amount, operation: Operation::by_id(operation) };
        match kind {
            "transient" => instance.add_transient_modifier(modifier),
            "permanent" => instance.add_permanent_modifier(modifier),
            _ => assert!(instance.remove_modifier(&id)),
        }
    }
    let crisp_protocol::nbt::Nbt::List(packed) = map.to_nbt() else { unreachable!() };
    let value = map.value(attribute);
    assert_eq!(format!("{:x} {}", value.to_bits(), to_string(&packed[0])), oracle(&format!("attr {label} ")), "{label}");
}

#[test]
fn attribute_values_sum_in_fastutil_order() {
    attribute(
        "speed",
        Attribute::MovementSpeed,
        0.1f32 as f64,
        &[
            ("transient", "minecraft:sprinting", 0.3f32 as f64, 2),
            ("permanent", "minecraft:effect.speed", 0.20000000298023224 * 3.0, 2),
            ("permanent", "minecraft:effect.slowness", -0.15000000596046448 * 2.0, 2),
        ],
    );
    let ids: Vec<String> = (0..40).map(|i| format!("test:m{i}")).collect();
    let mut many: Vec<(&str, &str, f64, u8)> = (0..40)
        .map(|i| (if i % 3 == 0 { "transient" } else { "permanent" }, ids[i].as_str(), 0.001 * i as f64 + 1.0e-12 * (i * 7 % 11) as f64, (i % 3) as u8))
        .collect();
    many.extend((0..40).step_by(3).map(|i| ("remove", ids[i].as_str(), 0.0, 0)));
    many.push(("permanent", "z:last", 0.3, 0));
    attribute("many", Attribute::Luck, 1.0, &many);
    attribute(
        "sums",
        Attribute::Armor,
        0.1,
        &[("permanent", "a", 0.1, 0), ("permanent", "b", 0.2, 0), ("permanent", "c", 1.0e16, 0), ("permanent", "d", -1.0e16, 0), ("permanent", "e", 0.3, 0), (
            "permanent",
            "f",
            3.0e-17,
            0,
        )],
    );
    attribute("clamped", Attribute::MaxHealth, 20.0, &[("permanent", "big", 5000.0, 0)]);
}

#[test]
fn attributes_load_what_they_save() {
    let mut map = AttributeMap::new(&PLAYER_ATTRIBUTES);
    let speed = map.instance(Attribute::MovementSpeed).unwrap();
    speed.add_permanent_modifier(AttributeModifier { id: Cow::Borrowed("minecraft:effect.speed"), amount: 0.4, operation: Operation::AddMultipliedTotal });
    speed.add_transient_modifier(AttributeModifier { id: Cow::Borrowed("minecraft:sprinting"), amount: 0.3, operation: Operation::AddMultipliedTotal });
    map.instance(Attribute::MaxHealth).unwrap().set_base_value(30.0);
    let saved = map.to_nbt();
    let mut loaded = AttributeMap::new(&PLAYER_ATTRIBUTES);
    loaded.load(&saved);
    assert_eq!(loaded.to_nbt(), saved);
    assert!(!loaded.has_modifier(Attribute::MovementSpeed, "minecraft:sprinting"));
    assert_eq!(loaded.value(Attribute::MaxHealth), 30.0);
    assert!(loaded.instance(Attribute::FlyingSpeed).is_none());
}
