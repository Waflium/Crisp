use crisp_core::random::LegacyRandomSource;
use crisp_core::{BlockPos, ChunkPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_data::{blocks, damage_types, game_rule, items};
use crisp_protocol::Reader;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::network::connection::Outbound;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::damagesource::DamageSource;
use crate::world::damagesource::combat_rules::damage_after_absorb;
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::food::FoodData;
use crate::world::item::ItemStack;
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
    let (out, packets) = Outbound::capture();
    level.player_mut(id).connection = out;
    (level, id, packets)
}

fn packet_ids(packets: &mut UnboundedReceiver<Vec<u8>>) -> Vec<i32> {
    let mut out = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        out.push(Reader::new(&packet).varint().unwrap());
    }
    out
}

fn tick(level: &mut ServerLevel, id: EntityId) {
    level.tick();
    level.tick_connection(id);
}

fn mob_attack() -> DamageSource {
    DamageSource::new(damage_types::MOB_ATTACK)
}

#[test]
fn armor_absorbs_damage_as_vanilla() {
    let cases: [(f32, f32, f32, u32); 8] = [
        (20.0, 20.0, 8.0, 1090519039),
        (10.0, 20.0, 8.0, 1077936128),
        (5.0, 15.0, 0.0, 1075838976),
        (4.0, 7.0, 0.0, 1078774989),
        (1.0, 20.0, 12.0, 1045757428),
        (50.0, 20.0, 8.0, 1108082688),
        (7.0, 20.0, 8.0, 1072819076),
        (6.0, 3.0, 0.0, 1086022746),
    ];
    for (damage, armor, toughness, bits) in cases {
        assert_eq!(damage_after_absorb(damage, armor, toughness).to_bits(), bits, "{damage} {armor} {toughness}");
    }
}

#[test]
fn worn_diamond_armor_protects_and_wears() {
    let (mut level, id, _packets) = setup();
    let armor = [(5, items::DIAMOND_HELMET), (6, items::DIAMOND_CHESTPLATE), (7, items::DIAMOND_LEGGINGS), (8, items::DIAMOND_BOOTS)];
    for (slot, item) in armor {
        level.player_mut(id).inventory.slots[slot] = Some(ItemStack::new(item, 1));
    }
    level.detect_equipment_updates(id);
    assert_eq!(level.armor_value(id), 20);
    assert_eq!(level.attribute_value(id, Attribute::ArmorToughness), 8.0);
    assert!(level.hurt_server(id, mob_attack(), 10.0));
    assert_eq!(level.player(id).health, 17.0);
    for (slot, _) in armor {
        assert_eq!(level.player(id).inventory.slots[slot].as_ref().unwrap().damage_value(), 2);
    }
    level.player_mut(id).inventory.slots[6] = None;
    level.detect_equipment_updates(id);
    assert_eq!(level.armor_value(id), 12);
    level.player_mut(id).living.damage_cooldown_time = 0;
    assert!(level.hurt_server(id, DamageSource::new(damage_types::FALL), 4.0));
    assert_eq!(level.player(id).health, 13.0);
}

#[test]
fn hurt_packets_and_knockback_match_vanilla() {
    let (mut level, id, mut packets) = setup();
    level.entity_mut(id).random = LegacyRandomSource::new(42);
    level.entity_mut(id).delta_movement = Vec3::new(0.0, -0.0784000015258789, 0.0);
    assert!(level.hurt_server(id, mob_attack(), 1.0));
    let m = level.entity(id).delta_movement;
    assert_eq!((m.x.to_bits(), m.y.to_bits(), m.z.to_bits()), (4597116478439476488, 4600171214993757307, 13823054353354310719));
    assert_eq!(level.player(id).living.hurt_dir, 0.0);
    assert_eq!(level.player(id).health, 19.0);
    assert_eq!(packet_ids(&mut packets), [clientbound::DAMAGE_EVENT, clientbound::HURT_ANIMATION]);
    assert_eq!(level.player(id).food_data.exhaustion_level, 0.1);
    assert!(!level.hurt_server(id, mob_attack(), 1.0));
    assert!(level.hurt_server(id, mob_attack(), 3.0));
    assert_eq!(level.player(id).health, 17.0);
    assert_eq!(packet_ids(&mut packets), Vec::<i32>::new());
}

#[test]
fn creative_players_and_unloaded_clients_are_not_hurt() {
    let (mut level, id, _packets) = setup();
    level.set_game_mode(id, crate::server::level::game_mode::GameType::Creative);
    assert!(!level.hurt_server(id, mob_attack(), 5.0));
    assert!(level.hurt_server(id, DamageSource::new(damage_types::OUT_OF_WORLD), 5.0));
    assert_eq!(level.player(id).health, 15.0);
    let (mut level, id, _packets) = setup();
    level.player_mut(id).movement.client_loaded_timeout_timer = 60;
    assert!(!level.hurt_server(id, DamageSource::new(damage_types::GENERIC_KILL), f32::MAX));
}

#[test]
fn fall_damage_is_applied_on_landing() {
    let (mut level, id, mut packets) = setup();
    level.entity_mut(id).fall_distance = 10.0;
    let pos = BlockPos::new(0, -61, 0);
    let state = crate::world::level::BlockGetter::block_state(&mut level, pos);
    level.check_fall_damage(id, 0.0, true, state, pos);
    assert_eq!(level.player(id).health, 13.0);
    assert_eq!(level.entity(id).fall_distance, 0.0);
    assert_eq!(packet_ids(&mut packets), [clientbound::LEVEL_PARTICLES, clientbound::DAMAGE_EVENT]);
    tick(&mut level, id);
    let ids = packet_ids(&mut packets);
    assert!(ids.contains(&clientbound::SET_ENTITY_MOTION));
    assert!(ids.contains(&clientbound::SET_HEALTH));
    level.game_rules.set(game_rule::FALL_DAMAGE, 0);
    level.entity_mut(id).fall_distance = 10.0;
    level.check_fall_damage(id, 0.0, true, state, pos);
    assert_eq!(level.player(id).health, 13.0);
}

#[test]
fn eating_fills_food_and_saturation() {
    let mut food = FoodData { food_level: 10, saturation_level: 0.0, ..FoodData::default() };
    let beef = items::COOKED_BEEF.info().food.unwrap();
    food.eat(&beef);
    assert_eq!((food.food_level, food.saturation_level), (18, 12.8));
    food.eat(&beef);
    assert_eq!((food.food_level, food.saturation_level), (20, 20.0));
}

#[test]
fn natural_regeneration_follows_saturation() {
    let (mut level, id, _packets) = setup();
    level.difficulty = Difficulty::Normal;
    level.set_health(id, 10.0);
    let heals: [(i32, u32); 10] = [
        (10, 1093490005),
        (20, 1094189056),
        (30, 1094713344),
        (40, 1095237632),
        (50, 1095587157),
        (60, 1095761920),
        (70, 1095936683),
        (80, 1096111446),
        (90, 1096286209),
        (170, 1097334785),
    ];
    let mut seen = Vec::new();
    for t in 1..=200 {
        let before = level.player(id).health;
        level.tick_food(id);
        let after = level.player(id).health;
        if after != before {
            seen.push((t, after.to_bits()));
        }
    }
    assert_eq!(seen, heals);
    let food = level.player(id).food_data;
    assert_eq!((food.food_level, food.saturation_level, food.exhaustion_level), (19, 0.0, 3.0));
    level.game_rules.set(game_rule::NATURAL_HEALTH_REGENERATION, 0);
    level.player_mut(id).food_data.food_level = 20;
    for _ in 0..200 {
        level.tick_food(id);
    }
    assert_eq!(level.player(id).health.to_bits(), 1097334785);
}

#[test]
fn starvation_depends_on_difficulty() {
    for (difficulty, floor) in [(Difficulty::Easy, 10.0), (Difficulty::Normal, 1.0), (Difficulty::Hard, 0.0)] {
        let (mut level, id, _packets) = setup();
        level.difficulty = difficulty;
        level.player_mut(id).food_data = FoodData { food_level: 0, saturation_level: 0.0, ..FoodData::default() };
        for _ in 0..80 {
            tick(&mut level, id);
        }
        assert_eq!(level.player(id).health, 19.0, "{difficulty:?}");
        for _ in 0..80 * 20 {
            tick(&mut level, id);
        }
        assert_eq!(level.player(id).health, floor, "{difficulty:?}");
    }
}

#[test]
fn sprinting_and_jumping_tire() {
    let (mut level, id, _packets) = setup();
    level.jump_from_ground(id);
    assert_eq!(level.player(id).food_data.exhaustion_level, 0.05);
    level.set_sprinting(id, true);
    level.jump_from_ground(id);
    assert_eq!(level.player(id).food_data.exhaustion_level, 0.05 + 0.2);
    let before = level.player(id).food_data.exhaustion_level;
    level.check_movement_statistics(id, 1.0, 0.0, 0.0);
    assert_eq!(level.player(id).food_data.exhaustion_level, before + 0.1 * 100.0 * 0.01);
}

#[test]
fn eating_takes_its_time() {
    let (mut level, id, mut packets) = setup();
    level.player_mut(id).food_data = FoodData { food_level: 10, saturation_level: 0.0, ..FoodData::default() };
    let slot = level.player(id).inventory.selected_slot();
    level.player_mut(id).inventory.slots[slot] = Some(ItemStack::new(items::COOKED_BEEF, 2));
    level.use_item(id, InteractionHand::MainHand);
    assert!(level.is_using_item(id));
    packet_ids(&mut packets);
    for _ in 0..31 {
        tick(&mut level, id);
    }
    assert_eq!(level.player(id).food_data.food_level, 10);
    tick(&mut level, id);
    assert!(!level.is_using_item(id));
    let food = level.player(id).food_data;
    assert_eq!((food.food_level, food.saturation_level), (18, 12.8));
    assert_eq!(level.player(id).inventory.slots[slot], Some(ItemStack::new(items::COOKED_BEEF, 1)));
    let ids = packet_ids(&mut packets);
    assert!(ids.contains(&clientbound::ENTITY_EVENT));
    level.player_mut(id).food_data.food_level = 20;
    level.use_item(id, InteractionHand::MainHand);
    assert!(!level.is_using_item(id));
    level.player_mut(id).inventory.slots[slot] = Some(ItemStack::new(items::GOLDEN_APPLE, 1));
    level.use_item(id, InteractionHand::MainHand);
    assert!(level.is_using_item(id));
}

#[test]
fn stew_leaves_its_bowl() {
    let (mut level, id, _packets) = setup();
    level.player_mut(id).food_data.food_level = 10;
    let slot = level.player(id).inventory.selected_slot();
    level.player_mut(id).inventory.slots[slot] = Some(ItemStack::new(items::MUSHROOM_STEW, 1));
    level.use_item(id, InteractionHand::MainHand);
    for _ in 0..32 {
        tick(&mut level, id);
    }
    assert_eq!(level.player(id).inventory.slots[slot], Some(ItemStack::new(items::BOWL, 1)));
    assert_eq!(level.player(id).food_data.food_level, 16);
}

#[test]
fn drowning_runs_out_of_air() {
    let (mut level, id, _packets) = setup();
    for y in [-60, -59] {
        level.set_block(BlockPos::new(0, y, 0), blocks::WATER.default_state(), UPDATE_ALL);
    }
    let mut first_hurt = None;
    for t in 1..=400 {
        tick(&mut level, id);
        if first_hurt.is_none() && level.player(id).health < 20.0 {
            first_hurt = Some(t);
            assert_eq!(level.entity(id).air_supply(), 0);
        }
    }
    assert_eq!(first_hurt, Some(320));
    assert!(level.player(id).health < 20.0);
}

#[test]
fn falling_out_of_the_world_hurts_every_ten_ticks() {
    let (mut level, id, _packets) = setup();
    level.player_mut(id).food_data = FoodData { food_level: 17, saturation_level: 0.0, ..FoodData::default() };
    let pos = Vec3::new(0.5, -200.0, 0.5);
    level.player_mut(id).entity.set_pos(pos);
    level.player_mut(id).reset_position();
    let mut hurt = Vec::new();
    for t in 1..=30 {
        let before = level.player(id).health;
        tick(&mut level, id);
        if level.player(id).health != before {
            hurt.push((t, level.player(id).health));
        }
    }
    assert_eq!(hurt, [(1, 16.0), (11, 12.0), (21, 8.0)]);
}

#[test]
fn suffocating_in_a_wall() {
    let (mut level, id, _packets) = setup();
    level.set_block(BlockPos::new(0, -59, 0), blocks::STONE.default_state(), UPDATE_ALL);
    assert!(level.is_in_wall(id));
    tick(&mut level, id);
    assert_eq!(level.player(id).health, 19.0);
}

#[test]
fn lava_burns() {
    let (mut level, id, _packets) = setup();
    level.set_block(BlockPos::new(0, -60, 0), blocks::LAVA.default_state(), UPDATE_ALL);
    tick(&mut level, id);
    tick(&mut level, id);
    assert_eq!(level.player(id).health, 16.0);
    assert!(level.entity(id).remaining_fire_ticks > 0);
    assert!(level.entity(id).shared_flag(crate::world::entity::flags::ON_FIRE));
}

#[test]
fn death_drops_the_inventory_unless_kept() {
    for keep in [false, true] {
        let (mut level, id, mut packets) = setup();
        level.game_rules.set(game_rule::KEEP_INVENTORY, keep as i32);
        level.player_mut(id).inventory.slots[36] = Some(ItemStack::new(items::STONE, 5));
        level.player_mut(id).inventory.slots[6] = Some(ItemStack::new(items::DIAMOND_CHESTPLATE, 1));
        level.kill(id);
        assert_eq!(level.player(id).health, 0.0);
        let items = level.entities.values().filter(|e| e.as_item().is_some()).count();
        assert_eq!(items, if keep { 0 } else { 2 });
        let mut sent = Vec::new();
        while let Ok(packet) = packets.try_recv() {
            sent.push(packet);
        }
        let id_of = |p: &Vec<u8>| Reader::new(p).varint().unwrap();
        let kill = sent.iter().position(|p| id_of(p) == clientbound::PLAYER_COMBAT_KILL).unwrap();
        assert_eq!(id_of(&sent[kill + 1]), clientbound::SYSTEM_CHAT);
        let contains = |p: &[u8], s: &[u8]| p.windows(s.len()).any(|w| w == s);
        assert!(contains(&sent[kill], b"death.attack.genericKill"));
        assert_eq!(id_of(sent.last().unwrap()), clientbound::ENTITY_EVENT);
        assert!(level.player(id).movement.waiting_for_respawn);
        for _ in 0..20 {
            tick(&mut level, id);
        }
        assert!(level.entity(id).is_removed());
        assert!(level.level_players().is_empty());
    }
}
