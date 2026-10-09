use crisp_core::Vec3;
use crisp_data::mob_effects;

use crate::commands::brigadier::{self, ContextChain, StringReader};
use crate::commands::source::CommandSourceStack;
use crate::commands::{node_at, perform_prefixed_command};
use crate::network::connection::Outbound;
use crate::network::login::{Profile, offline_uuid};
use crate::server::Server;
use crate::server::level::testing;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;

fn server() -> Server {
    Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap()
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

fn exec(server: &mut Server, command: &str) -> Result<i32, String> {
    let source = CommandSourceStack::server(server);
    let units: Vec<u16> = command.encode_utf16().collect();
    let parse = brigadier::parse(StringReader::new(&units), &source);
    assert!(!parse.reader.can_read(), "{command} does not parse: {:?}", parse.exceptions);
    let chain = ContextChain::try_flatten(parse.context.build(command)).expect("an executable command");
    let node = chain.executable.command.unwrap();
    let f = super::executors().into_iter().find(|(path, _)| node_at(path).id == node).expect("ported").1;
    f(server, &chain.executable, &source).map_err(|e| e.message.get_string())
}

fn join(server: &mut Server, name: &str) -> EntityId {
    let id = testing::player(&mut server.level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let player = server.level.player_mut(id);
    player.connection = Outbound::detached();
    player.profile = Profile { uuid: offline_uuid(name), name: name.into(), properties: Vec::new() };
    player.entity.uuid = player.profile.uuid;
    id
}

#[test]
fn effect_give() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    let bob = join(&mut server, "Bob");
    assert_eq!(run(&mut server, "effect give Alice minecraft:speed"), ["Applied effect Speed to Alice"]);
    let speed = server.level.player(alice).living.effect(mob_effects::SPEED).cloned().unwrap();
    assert_eq!((speed.duration, speed.amplifier, speed.visible, speed.show_icon), (600, 0, true, true));
    assert_eq!(run(&mut server, "effect give @a speed 30"), ["Applied effect Speed to Bob"]);
    assert_eq!(run(&mut server, "effect give @a speed 40 1 true"), ["Applied effect Speed to 2 targets"]);
    let speed = server.level.player(bob).living.effect(mob_effects::SPEED).cloned().unwrap();
    assert_eq!((speed.duration, speed.amplifier, speed.visible, speed.show_icon), (800, 1, false, false));
    assert_eq!(exec(&mut server, "effect give @a speed 40 1 true"), Err("Unable to apply this effect (target is either immune to effects, or has something stronger)".into()));
    assert_eq!(exec(&mut server, "effect give Alice haste infinite 3"), Ok(1));
    let haste = server.level.player(alice).living.effect(mob_effects::HASTE).cloned().unwrap();
    assert_eq!((haste.duration, haste.amplifier), (-1, 3));
    assert_eq!(run(&mut server, "effect give Alice haste infinite 2 false"), ["Unable to apply this effect (target is either immune to effects, or has something stronger)"]);
    assert_eq!(exec(&mut server, "effect give Alice instant_health 7"), Ok(1));
    assert_eq!(server.level.player(alice).living.effect(mob_effects::INSTANT_HEALTH).unwrap().duration, 7);
    assert_eq!(run(&mut server, "effect give Bob minecraft:saturation"), ["Applied effect Saturation to Bob"]);
    assert_eq!(server.level.player(bob).living.effect(mob_effects::SATURATION).unwrap().duration, 1);
    let item = ItemEntity::new(Vec3::new(0.5, -60.0, 0.5), ItemStack::new(crisp_data::items::STONE, 1));
    server.level.add_fresh_entity(LevelEntity::Item(item));
    assert_eq!(run(&mut server, "effect give @e[type=item] speed"), ["Unable to apply this effect (target is either immune to effects, or has something stronger)"]);
    assert_eq!(run(&mut server, "effect give Alice speed 0"), ["Integer must not be less than 1: found 0", "...ice speed 0<--[HERE]"]);
    assert_eq!(run(&mut server, "effect give Alice nope"), ["Can't find element 'minecraft:nope' of type 'minecraft:mob_effect'", "...Alice nope<--[HERE]"]);
}

#[test]
fn effect_clear() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    join(&mut server, "Bob");
    assert_eq!(run(&mut server, "effect clear @a"), ["Target has no effects to remove"]);
    assert_eq!(run(&mut server, "effect clear"), ["An entity is required to run this command here"]);
    run(&mut server, "effect give @a speed");
    run(&mut server, "effect give Alice glowing");
    assert_eq!(run(&mut server, "effect clear Alice glowing"), ["Removed effect Glowing from Alice"]);
    assert_eq!(run(&mut server, "effect clear Alice glowing"), ["Target doesn't have the requested effect"]);
    assert_eq!(exec(&mut server, "effect clear @a minecraft:speed"), Ok(2));
    run(&mut server, "effect give @a speed");
    assert_eq!(run(&mut server, "effect clear @a"), ["Removed every effect from 2 targets"]);
    run(&mut server, "effect give Alice speed");
    assert_eq!(run(&mut server, "effect clear Alice"), ["Removed every effect from Alice"]);
    assert!(server.level.player(alice).living.active_effects.is_empty());
    assert_eq!(server.level.attribute_value(alice, Attribute::MovementSpeed), 0.1f32 as f64);
}

#[test]
fn attribute() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    assert_eq!(run(&mut server, "attribute Alice max_health get"), ["The value of attribute Max Health for entity Alice is 20.0"]);
    assert_eq!(exec(&mut server, "attribute Alice movement_speed get 1000"), Ok(100));
    assert_eq!(run(&mut server, "attribute Alice movement_speed base get"), ["The base value of attribute Speed for entity Alice is 0.10000000149011612"]);
    assert_eq!(run(&mut server, "attribute Alice flying_speed get"), ["Entity Alice has no attribute Flying Speed"]);
    assert_eq!(run(&mut server, "attribute Alice max_health base set 30"), ["The base value for attribute Max Health for entity Alice set to 30.0"]);
    assert_eq!(server.level.max_health(alice), 30.0);
    assert_eq!(run(&mut server, "attribute Alice max_health modifier add test:boost 2 add_multiplied_base"), [
        "Added modifier test:boost to attribute Max Health for entity Alice"
    ]);
    assert_eq!(run(&mut server, "attribute Alice max_health modifier add test:boost 2 add_value"), [
        "Modifier test:boost is already present on attribute Max Health for entity Alice"
    ]);
    assert_eq!(exec(&mut server, "attribute Alice max_health get"), Ok(90));
    assert_eq!(run(&mut server, "attribute Alice max_health modifier value get test:boost 10"), [
        "The value of modifier test:boost on attribute Max Health for entity Alice is 2.0"
    ]);
    assert_eq!(exec(&mut server, "attribute Alice max_health modifier value get test:boost 10"), Ok(20));
    assert_eq!(run(&mut server, "attribute Alice max_health modifier value get test:other"), ["Attribute Max Health for entity Alice has no modifier test:other"]);
    let saved = server.level.player(alice).to_nbt("minecraft:overworld");
    let attributes = format!("{:?}", saved.get("attributes").unwrap());
    assert!(attributes.contains("test:boost"), "{attributes}");
    assert_eq!(run(&mut server, "attribute Alice max_health modifier remove test:boost"), ["Removed modifier test:boost from attribute Max Health for entity Alice"]);
    assert_eq!(run(&mut server, "attribute Alice max_health modifier remove test:boost"), ["Attribute Max Health for entity Alice has no modifier test:boost"]);
    assert_eq!(run(&mut server, "attribute Alice max_health base reset"), ["The base value for attribute Max Health for entity Alice reset to default 20.0"]);
    assert_eq!(run(&mut server, "attribute Alice luck base reset"), ["The base value for attribute Luck for entity Alice reset to default 0.0"]);
    run(&mut server, "attribute Alice max_health base set 10");
    server.level.refresh_dirty_attributes(alice);
    assert_eq!(server.level.player(alice).health, 10.0);
    let item = ItemEntity::new(Vec3::new(0.5, -60.0, 0.5), ItemStack::new(crisp_data::items::STONE, 1));
    server.level.add_fresh_entity(LevelEntity::Item(item));
    assert_eq!(run(&mut server, "attribute @e[type=item,limit=1] luck get"), ["Stone is not a valid entity for this command"]);
}

#[test]
fn base_values_outlive_death_and_effects_do_not() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    run(&mut server, "attribute Alice max_health base set 30");
    run(&mut server, "attribute Alice max_health modifier add test:boost 4 add_value");
    run(&mut server, "effect give Alice speed");
    server.level.kill(alice);
    server.respawn(alice);
    assert_eq!(server.level.max_health(alice), 30.0);
    assert_eq!(server.level.player(alice).health, 30.0);
    assert!(server.level.player(alice).living.active_effects.is_empty());
}
