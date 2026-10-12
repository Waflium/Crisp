use crisp_core::Vec3;
use crisp_data::damage_types;
use crisp_data::packets::play::clientbound;
use crisp_protocol::Reader;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::network::connection::Outbound;
use crate::network::login::{Profile, offline_uuid};
use crate::server::Server;
use crate::server::level::testing;
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::{LevelType, ServerConfig};

fn server(world: Option<std::path::PathBuf>) -> Server {
    Server::new(ServerConfig { level_type: LevelType::Flat, world, ..ServerConfig::default() }).unwrap()
}

fn run_as(server: &mut Server, source: &CommandSourceStack, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    perform_prefixed_command(server, source, command);
    server.console_log.borrow().clone()
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    let source = CommandSourceStack::server(server);
    run_as(server, &source, command)
}

fn join(server: &mut Server, name: &str) -> (EntityId, UnboundedReceiver<Vec<u8>>) {
    let profile = Profile { uuid: offline_uuid(name), name: name.into(), properties: Vec::new() };
    let id = testing::player_with_profile(&mut server.level, profile, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let (out, packets) = Outbound::capture();
    server.level.player_mut(id).connection = out.clone();
    server.scoreboard.borrow_mut().server.add_connection(id, out);
    server.level.handle_player_loaded(id);
    (id, packets)
}

fn packet_ids(packets: &mut UnboundedReceiver<Vec<u8>>) -> Vec<i32> {
    std::iter::from_fn(|| packets.try_recv().ok()).map(|p| Reader::new(&p).varint().unwrap()).collect()
}

fn score(server: &Server, holder: &str, objective: &str) -> Option<i32> {
    let scoreboard = server.scoreboard.borrow();
    scoreboard.get_player_score_info(holder, scoreboard.get_objective(objective)?.id()).map(|s| s.value)
}

#[test]
fn teams_decorate_names_and_selectors_see_teams_and_scores() {
    let mut server = server(None);
    let (alice, _) = join(&mut server, "Alice");
    run(&mut server, "team add red");
    run(&mut server, "team modify red prefix \"[R] \"");
    run(&mut server, "team modify red suffix \"!\"");
    let alice_source = CommandSourceStack { permission: 2, ..CommandSourceStack::player(&server, alice) };
    run_as(&mut server, &alice_source, "team join red");
    assert_eq!(server.entity_display_name(alice).get_string(), "[R] Alice!");
    assert_eq!(server.entity_name(alice), "Alice");
    run(&mut server, "scoreboard objectives add foo dummy");
    assert_eq!(run(&mut server, "scoreboard players set @a foo 1"), ["Set [foo] for [R] Alice! to 1"]);
    assert_eq!(run(&mut server, "execute if entity @a[team=red]"), ["Test passed. Count: 1"]);
    assert_eq!(run(&mut server, "execute if entity @a[team=!red]"), ["Test failed"]);
    assert_eq!(run(&mut server, "execute if entity @a[team=]"), ["Test failed"]);
    assert_eq!(run(&mut server, "execute if entity @a[name=Alice]"), ["Test passed. Count: 1"]);
    assert_eq!(run(&mut server, "execute if entity @a[scores={foo=1}]"), ["Test passed. Count: 1"]);
    assert_eq!(run(&mut server, "execute if entity @a[scores={foo=2..,foo=..1}]"), ["Test passed. Count: 1"]);
    assert_eq!(run(&mut server, "execute if entity @a[scores={nope=1}]"), ["Test failed"]);
    run(&mut server, "team leave Alice");
    assert_eq!(server.entity_display_name(alice).get_string(), "Alice");
    assert_eq!(run(&mut server, "execute if entity @a[team=]"), ["Test passed. Count: 1"]);
}

#[test]
fn friendly_fire_follows_the_team() {
    let mut server = server(None);
    let (alice, _) = join(&mut server, "Alice");
    let (bob, _) = join(&mut server, "Bob");
    run(&mut server, "team add red");
    run(&mut server, "team join red @a[name=!Carol]");
    run(&mut server, "team modify red friendlyFire false");
    let attack = DamageSource { causing_entity: Some(bob), direct_entity: Some(bob), ..DamageSource::new(damage_types::MOB_ATTACK) };
    assert!(!server.level.hurt_server(alice, attack, 2.0));
    run(&mut server, "team leave Bob");
    assert!(server.level.hurt_server(alice, attack, 2.0));
}

#[test]
fn criteria_keep_scores_and_deaths_are_told_to_whom_the_team_says() {
    let mut server = server(None);
    for (name, criterion) in [("hp", "health"), ("food", "food"), ("air", "air"), ("armor", "armor"), ("xp", "xp"), ("level", "level"), ("deaths", "deathCount")] {
        run(&mut server, &format!("scoreboard objectives add {name} {criterion}"));
    }
    let (alice, _) = join(&mut server, "Alice");
    let (_, mut bob) = join(&mut server, "Bob");
    let (_, mut carol) = join(&mut server, "Carol");
    server.level.tick_connection(alice);
    let values: Vec<Option<i32>> = ["hp", "food", "air", "armor", "xp", "level", "deaths"].iter().map(|o| score(&server, "Alice", o)).collect();
    assert_eq!(values, [Some(20), Some(20), Some(300), Some(0), Some(0), Some(0), None]);
    run(&mut server, "team add red");
    run(&mut server, "team join red @a[name=!Carol]");
    run(&mut server, "team modify red deathMessageVisibility hideForOtherTeams");
    packet_ids(&mut bob);
    packet_ids(&mut carol);
    server.level.kill(alice);
    assert_eq!(score(&server, "Alice", "deaths"), Some(1));
    assert!(packet_ids(&mut bob).contains(&clientbound::SYSTEM_CHAT), "teammates hear of it");
    assert!(!packet_ids(&mut carol).contains(&clientbound::SYSTEM_CHAT), "others don't");
}

#[test]
fn players_trigger_enabled_objectives_once() {
    let mut server = server(None);
    let (alice, _) = join(&mut server, "Alice");
    run(&mut server, "scoreboard objectives add t trigger");
    run(&mut server, "scoreboard objectives add d dummy");
    let source = CommandSourceStack::player(&server, alice);
    run_as(&mut server, &source, "trigger t");
    assert_eq!(score(&server, "Alice", "t"), None, "not enabled");
    run(&mut server, "scoreboard players enable Alice t");
    run_as(&mut server, &source, "trigger t add 5");
    assert_eq!(score(&server, "Alice", "t"), Some(5));
    run_as(&mut server, &source, "trigger t set 9");
    assert_eq!(score(&server, "Alice", "t"), Some(5));
    run(&mut server, "scoreboard players enable @a t");
    run_as(&mut server, &source, "trigger t");
    assert_eq!(score(&server, "Alice", "t"), Some(6));
    run(&mut server, "scoreboard players enable Alice d");
    run_as(&mut server, &source, "trigger d");
    assert_eq!(score(&server, "Alice", "d"), None);
}

#[test]
fn removed_entities_lose_their_scores_and_team() {
    let mut server = server(None);
    join(&mut server, "Alice");
    let item = ItemEntity::new(Vec3::new(0.5, -60.0, 0.5), ItemStack::new(crisp_data::items::STONE, 1));
    server.level.add_fresh_entity(LevelEntity::Item(item));
    run(&mut server, "scoreboard objectives add foo dummy");
    run(&mut server, "team add red");
    run(&mut server, "scoreboard players set @e[type=item] foo 4");
    run(&mut server, "team join red @e[type=item]");
    assert_eq!(server.scoreboard.borrow().tracked_players().len(), 1);
    run(&mut server, "kill @e[type=item]");
    assert!(server.scoreboard.borrow().tracked_players().is_empty());
    assert_eq!(server.scoreboard.borrow().get_player_team("red").unwrap().player_count(), 0);
}

#[test]
fn scoreboard_is_saved_and_loaded() {
    let dir = std::env::temp_dir().join(format!("crisp-scoreboard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut first = server(Some(dir.clone()));
    run(&mut first, "scoreboard objectives add foo dummy {text:\"Foo\",color:\"red\"}");
    run(&mut first, "scoreboard objectives setdisplay sidebar foo");
    run(&mut first, "scoreboard players set #x foo 12");
    run(&mut first, "team add red");
    run(&mut first, "team join red #x");
    first.save_everything(true, true, true);
    assert!(dir.join("data/minecraft/scoreboard.dat").exists());
    let mut second = server(Some(dir.clone()));
    assert_eq!(run(&mut second, "scoreboard players get #x foo"), ["#x has 12 [Foo]"]);
    assert_eq!(run(&mut second, "team list red"), ["Team [red] has 1 member(s): #x"]);
    assert_eq!(run(&mut second, "scoreboard objectives setdisplay sidebar foo"), ["Nothing changed. That display slot is already showing that objective"]);
    std::fs::remove_dir_all(dir).unwrap();
}
