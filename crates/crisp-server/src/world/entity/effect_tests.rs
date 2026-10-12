use crisp_core::{BlockPos, ChunkPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_data::{blocks, damage_types, game_rule, items, mob_effects, registry, registry_id};
use crisp_protocol::{Reader, Writer};
use tokio::sync::mpsc::UnboundedReceiver;

use super::EntityId;
use super::attributes::Attribute;
use super::data::{DataValue, ids};
use crate::network::connection::Outbound;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::flags;
use crate::world::food::FoodData;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

fn setup() -> (ServerLevel, EntityId, UnboundedReceiver<Vec<u8>>) {
    let mut level = testing::level();
    for x in -2..=2 {
        for z in -2..=2 {
            level.chunks.chunk(ChunkPos::new(x, z));
        }
    }
    let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.handle_player_loaded(id);
    level.entity_mut(id).on_ground = true;
    level.game_rules.set(game_rule::NATURAL_HEALTH_REGENERATION, 0);
    let (out, packets) = Outbound::capture();
    level.player_mut(id).connection = out;
    (level, id, packets)
}

fn packets(packets: &mut UnboundedReceiver<Vec<u8>>) -> Vec<(i32, Vec<u8>)> {
    let mut out = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        let mut r = Reader::new(&packet);
        let id = r.varint().unwrap();
        out.push((id, packet[packet.len() - r.remaining()..].to_vec()));
    }
    out
}

fn tick(level: &mut ServerLevel, id: EntityId) {
    level.tick();
    level.tick_connection(id);
}

fn give(level: &mut ServerLevel, id: EntityId, effect: crisp_data::MobEffect, duration: i32, amplifier: i32) -> bool {
    level.add_effect(id, MobEffectInstance::new(effect, duration, amplifier), None)
}

fn health_changes(level: &mut ServerLevel, id: EntityId, ticks: i32) -> Vec<(i32, f32)> {
    let mut changes = Vec::new();
    for t in 1..=ticks {
        let before = level.player(id).health;
        tick(level, id);
        if level.player(id).health != before {
            changes.push((t, level.player(id).health));
        }
    }
    changes
}

#[test]
fn regeneration_heals_every_50_ticks_halved_per_level() {
    let (mut level, id, _packets) = setup();
    level.set_health(id, 10.0);
    give(&mut level, id, mob_effects::REGENERATION, 100, 0);
    assert_eq!(health_changes(&mut level, id, 101), [(1, 11.0), (51, 12.0)]);
    assert!(!level.has_effect(id, mob_effects::REGENERATION));
    give(&mut level, id, mob_effects::REGENERATION, 30, 1);
    assert_eq!(health_changes(&mut level, id, 30), [(6, 13.0)]);
}

#[test]
fn poison_stops_at_one_health_and_wither_does_not() {
    let (mut level, id, _packets) = setup();
    level.set_health(id, 3.0);
    give(&mut level, id, mob_effects::POISON, 100, 0);
    assert_eq!(health_changes(&mut level, id, 100), [(1, 2.0), (26, 1.0)]);
    give(&mut level, id, mob_effects::WITHER, 41, 0);
    assert_eq!(health_changes(&mut level, id, 41), [(2, 0.0)]);
}

#[test]
fn instant_effects_act_once() {
    let (mut level, id, _packets) = setup();
    give(&mut level, id, mob_effects::INSTANT_DAMAGE, 1, 1);
    assert_eq!(health_changes(&mut level, id, 3), [(1, 8.0)]);
    give(&mut level, id, mob_effects::INSTANT_HEALTH, 1, 0);
    assert_eq!(health_changes(&mut level, id, 3), [(1, 12.0)]);
    assert!(level.player(id).living.active_effects.is_empty());
}

#[test]
fn hunger_and_saturation_feed_on_food_data() {
    let (mut level, id, _packets) = setup();
    level.player_mut(id).food_data = FoodData { food_level: 10, saturation_level: 0.0, ..FoodData::default() };
    give(&mut level, id, mob_effects::SATURATION, 1, 1);
    tick(&mut level, id);
    let food = level.player(id).food_data;
    assert_eq!((food.food_level, food.saturation_level), (12, 4.0));
    give(&mut level, id, mob_effects::HUNGER, 10, 2);
    let before = level.player(id).food_data.exhaustion_level;
    tick(&mut level, id);
    assert_eq!(level.player(id).food_data.exhaustion_level, before + 0.005 * 3.0);
}

#[test]
fn health_boost_and_absorption_follow_their_attributes() {
    let (mut level, id, mut sent) = setup();
    give(&mut level, id, mob_effects::HEALTH_BOOST, 200, 1);
    assert_eq!(level.max_health(id), 28.0);
    level.set_health(id, 28.0);
    give(&mut level, id, mob_effects::ABSORPTION, 200, 1);
    assert_eq!((level.max_absorption(id), level.absorption_amount(id)), (8.0, 8.0));
    level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 3.0);
    assert_eq!((level.absorption_amount(id), level.player(id).health), (5.0, 28.0));
    packets(&mut sent);
    tick(&mut level, id);
    assert!(packets(&mut sent).iter().any(|(id, _)| *id == clientbound::UPDATE_ATTRIBUTES));
    assert!(level.remove_effect(id, mob_effects::HEALTH_BOOST));
    assert_eq!(level.player(id).health, 20.0);
    assert!(level.remove_effect(id, mob_effects::ABSORPTION));
    assert_eq!(level.absorption_amount(id), 0.0);
    give(&mut level, id, mob_effects::ABSORPTION, 200, 0);
    level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 10.0);
    tick(&mut level, id);
    assert!(!level.has_effect(id, mob_effects::ABSORPTION));
}

#[test]
fn speed_and_strength_modify_attributes() {
    let (mut level, id, _packets) = setup();
    give(&mut level, id, mob_effects::SPEED, 100, 1);
    let speed = 0.1f32 as f64 * (1.0 + 0.20000000298023224 * 2.0);
    assert_eq!(level.attribute_value(id, Attribute::MovementSpeed), speed);
    give(&mut level, id, mob_effects::SPEED, 50, 2);
    assert_eq!(level.attribute_value(id, Attribute::MovementSpeed), 0.1f32 as f64 * (1.0 + 0.20000000298023224 * 3.0));
    give(&mut level, id, mob_effects::WEAKNESS, 100, 0);
    assert_eq!(level.attribute_value(id, Attribute::AttackDamage), 0.0);
    for _ in 0..50 {
        tick(&mut level, id);
    }
    assert_eq!(level.player(id).living.effect(mob_effects::SPEED).map(|e| (e.amplifier, e.duration)), Some((1, 50)));
    assert_eq!(level.attribute_value(id, Attribute::MovementSpeed), speed);
}

#[test]
fn the_client_hears_of_effects_coming_updating_and_going() {
    let (mut level, id, mut sent) = setup();
    packets(&mut sent);
    give(&mut level, id, mob_effects::SPEED, 20, 1);
    let update = |amplifier: i32, duration: i32, flags: u8| {
        let mut w = Writer::new();
        w.varint(id);
        w.varint(mob_effects::SPEED.protocol_id());
        w.varint(amplifier);
        w.varint(duration);
        w.u8(flags);
        (clientbound::UPDATE_MOB_EFFECT, w.into_inner())
    };
    assert_eq!(packets(&mut sent), [update(1, 20, 2 | 4 | 8)]);
    assert!(!give(&mut level, id, mob_effects::SPEED, 10, 0));
    assert!(!give(&mut level, id, mob_effects::SPEED, 20, 1));
    assert!(packets(&mut sent).is_empty());
    assert!(give(&mut level, id, mob_effects::SPEED, 25, 1));
    assert_eq!(packets(&mut sent), [update(1, 25, 2 | 4)]);
    level.add_effect(id, MobEffectInstance::with(mob_effects::SPEED, 30, 1, true, false, true), None);
    assert_eq!(packets(&mut sent), [update(1, 30, 1 | 4)]);
    for _ in 0..30 {
        tick(&mut level, id);
    }
    let mut w = Writer::new();
    w.varint(id);
    w.varint(mob_effects::SPEED.protocol_id());
    let sent = packets(&mut sent);
    assert!(sent.contains(&(clientbound::REMOVE_MOB_EFFECT, w.into_inner())));
    assert!(level.player(id).living.active_effects.is_empty());
}

#[test]
fn effects_show_in_entity_data() {
    let (mut level, id, _packets) = setup();
    give(&mut level, id, mob_effects::INVISIBILITY, 100, 0);
    give(&mut level, id, mob_effects::GLOWING, 100, 0);
    level.add_effect(id, MobEffectInstance::with(mob_effects::TRIAL_OMEN, 100, 0, true, true, true), None);
    tick(&mut level, id);
    let e = &level.player(id).entity;
    assert!(e.shared_flag(flags::INVISIBLE) && e.shared_flag(flags::GLOWING));
    let DataValue::Particles(particles) = e.data.get(ids::EFFECT_PARTICLES) else { panic!() };
    let entity_effect = registry_id(registry::PARTICLE_TYPE, "minecraft:entity_effect");
    let colors: Vec<_> = particles.iter().map(|p| (p.particle_type, p.color)).collect();
    assert_eq!(
        colors,
        [
            (entity_effect, Some(255 << 24 | 16185078)),
            (entity_effect, Some(255 << 24 | 9740385)),
            (registry_id(registry::PARTICLE_TYPE, "minecraft:trial_omen"), None)
        ]
    );
    assert_eq!(e.data.get(ids::EFFECT_AMBIENCE), &DataValue::Boolean(false));
    level.remove_all_effects(id);
    tick(&mut level, id);
    let e = &level.player(id).entity;
    assert!(!e.shared_flag(flags::INVISIBLE) && !e.shared_flag(flags::GLOWING));
    assert_eq!(e.data.get(ids::EFFECT_PARTICLES), &DataValue::Particles(Vec::new()));
}

#[test]
fn damage_effects() {
    let (mut level, id, _packets) = setup();
    give(&mut level, id, mob_effects::RESISTANCE, 100, 0);
    level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 10.0);
    assert_eq!(level.player(id).health, 12.0);
    give(&mut level, id, mob_effects::FIRE_RESISTANCE, 100, 0);
    assert!(!level.hurt_server(id, DamageSource::new(damage_types::LAVA), 4.0));
    level.set_block(BlockPos::new(0, -60, 0), blocks::LAVA.default_state(), UPDATE_ALL);
    for _ in 0..20 {
        tick(&mut level, id);
    }
    assert_eq!(level.player(id).health, 12.0);
}

#[test]
fn food_milk_honey_and_totems() {
    let (mut level, id, _packets) = setup();
    let slot = level.player(id).inventory.selected_slot();
    level.player_mut(id).inventory.slots[slot] = Some(ItemStack::new(items::GOLDEN_APPLE, 1));
    level.use_item(id, InteractionHand::MainHand);
    for _ in 0..32 {
        tick(&mut level, id);
    }
    let effects: Vec<_> = level.player(id).living.active_effects.iter().map(|e| (e.effect, e.amplifier, e.duration)).collect();
    assert_eq!(effects, [(mob_effects::REGENERATION, 1, 100), (mob_effects::ABSORPTION, 0, 2400)]);
    assert_eq!(level.absorption_amount(id), 4.0);
    give(&mut level, id, mob_effects::POISON, 100, 0);
    for effect in items::HONEY_BOTTLE.info().consumable.as_ref().unwrap().on_consume_effects {
        level.apply_consume_effect(id, effect);
    }
    assert!(!level.has_effect(id, mob_effects::POISON) && level.has_effect(id, mob_effects::REGENERATION));
    for effect in items::MILK_BUCKET.info().consumable.as_ref().unwrap().on_consume_effects {
        level.apply_consume_effect(id, effect);
    }
    assert!(level.player(id).living.active_effects.is_empty());
    give(&mut level, id, mob_effects::WITHER, 100, 0);
    level.player_mut(id).inventory.slots[slot] = Some(ItemStack::new(items::TOTEM_OF_UNDYING, 1));
    level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 100.0);
    assert_eq!(level.player(id).health, 1.0);
    let effects: Vec<_> = level.player(id).living.active_effects.iter().map(|e| (e.effect, e.amplifier, e.duration)).collect();
    assert_eq!(effects, [(mob_effects::REGENERATION, 1, 900), (mob_effects::FIRE_RESISTANCE, 0, 800), (mob_effects::ABSORPTION, 1, 100)]);
}

#[test]
fn weaving_leaves_cobwebs_where_its_player_died() {
    let (mut level, id, _packets) = setup();
    level.entity_mut(id).random = crisp_core::random::LegacyRandomSource::new(42);
    give(&mut level, id, mob_effects::WEAVING, 1000, 0);
    level.kill(id);
    for _ in 0..20 {
        tick(&mut level, id);
    }
    assert!(level.entity(id).is_removed());
    let mut cobwebs = 0;
    for x in -1..=1 {
        for y in -61..=-59 {
            for z in -1..=1 {
                cobwebs += level.block_state(BlockPos::new(x, y, z)).is_block(blocks::COBWEB) as i32;
            }
        }
    }
    assert!((2..=3).contains(&cobwebs), "{cobwebs} cobwebs");
}

#[test]
fn effects_and_attributes_are_saved_with_the_player() {
    let (mut level, id, _packets) = setup();
    give(&mut level, id, mob_effects::STRENGTH, 400, 0);
    give(&mut level, id, mob_effects::STRENGTH, 100, 2);
    give(&mut level, id, mob_effects::HEALTH_BOOST, -1, 0);
    level.set_health(id, 24.0);
    let nbt = level.player(id).to_nbt("minecraft:overworld");
    let names: Vec<&str> = match &nbt {
        crisp_protocol::nbt::Nbt::Compound(fields) => fields.iter().map(|(k, _)| k.as_str()).collect(),
        _ => unreachable!(),
    };
    let at = |key: &str| names.iter().position(|n| *n == key).unwrap();
    assert!(at("AbsorptionAmount") < at("attributes") && at("attributes") < at("active_effects") && at("active_effects") < at("DataVersion"));
    let mut other = testing::level();
    let loaded = testing::player(&mut other, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    other.player_mut(loaded).load_nbt(&nbt);
    assert_eq!(other.player(loaded).living.active_effects, level.player(id).living.active_effects);
    assert_eq!(other.max_health(loaded), 24.0);
    assert_eq!(other.player(loaded).health, 24.0);
    assert_eq!(other.attribute_value(loaded, Attribute::AttackDamage), 1.0 + 3.0 * 3.0);
    let saved = other.player(loaded).to_nbt("minecraft:overworld");
    assert_eq!(saved.get("attributes").unwrap().as_list().len(), 3);
    assert_eq!(saved.get("active_effects"), nbt.get("active_effects"));
}
