use crisp_core::Vec3;

use crate::advancements::triggers::criteria_triggers;
use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::network::login::{Profile, offline_uuid};
use crate::server::Server;
use crate::server::level::testing;
use crate::server::player_advancements::PlayerAdvancements;
use crate::server::server_advancement_manager::ServerAdvancementManager;
use crate::world::entity::EntityId;

fn server() -> Server {
    Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap()
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    let source = CommandSourceStack::server(server);
    server.console_log.borrow_mut().clear();
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

fn join(server: &mut Server, name: &str) -> EntityId {
    let profile = Profile { uuid: offline_uuid(name), name: name.into(), properties: Vec::new() };
    let id = testing::server_player_with_profile(server, profile, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    server.levels[0].player_mut(id).advancements = server.get_player_advancements(offline_uuid(name));
    server.name_to_id_cache.add(crate::server::players::NameAndId { id: offline_uuid(name), name: name.into() });
    id
}

fn is_done(server: &mut Server, player: EntityId, id: &str) -> bool {
    let advancement = ServerAdvancementManager::get_bound().get(id).expect("advancement");
    server.levels[0].player_mut(player).advancements.get_or_start_progress(advancement).is_done()
}

#[test]
fn grants_and_revokes_like_vanilla() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    let cases: &[(&str, &[&str])] = &[
        ("advancement grant Alice only minecraft:story/smelt_iron", &["Alice has made the advancement [Acquire Hardware]", "Granted the advancement [Acquire Hardware] to Alice"]),
        ("advancement grant Alice only minecraft:story/smelt_iron", &["Couldn't grant advancement [Acquire Hardware] to Alice as they already have it"]),
        ("advancement revoke Alice only minecraft:story/smelt_iron", &["Revoked the advancement [Acquire Hardware] from Alice"]),
        ("advancement revoke Alice only minecraft:story/smelt_iron", &["Couldn't revoke advancement [Acquire Hardware] from Alice as they don't have it"]),
        ("advancement grant Alice only minecraft:story/iron_tools iron_pickaxe", &["Alice has made the advancement [Isn't It Iron Pick?]", "Granted criterion 'iron_pickaxe' of advancement [Isn't It Iron Pick?] to Alice"]),
        ("advancement grant Alice only minecraft:story/iron_tools nope", &["The advancement [Isn't It Iron Pick?] does not contain the criterion 'nope'"]),
        ("advancement grant Alice only minecraft:nope", &["Unknown advancement: minecraft:nope"]),
        ("advancement grant Alice until minecraft:story/upgrade_tools", &["Alice has made the advancement [Stone Age]", "Alice has made the advancement [Getting an Upgrade]", "Granted 3 advancements to Alice"]),
    ];
    for (command, expected) in cases {
        assert_eq!(run(&mut server, command), *expected, "{command}");
    }
    assert!(is_done(&mut server, alice, "minecraft:story/root"));
    assert!(!is_done(&mut server, alice, "minecraft:story/smelt_iron"));
    run(&mut server, "gamerule show_advancement_messages false");
    assert_eq!(run(&mut server, "advancement grant Alice only minecraft:story/mine_diamond"), ["Granted the advancement [Diamonds!] to Alice"]);
}

#[test]
fn inventory_changes_award_advancements_and_recipes() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    server.levels[0].init_inventory_menu(alice);
    criteria_triggers::TICK.trigger(&mut server.levels[0], alice);
    assert!(is_done(&mut server, alice, "minecraft:recipes/decorations/crafting_table"));
    assert!(server.levels[0].player(alice).recipe_book.contains("minecraft:crafting_table"));
    assert_eq!(run(&mut server, "give Alice minecraft:cobblestone"), ["Gave 1 [Cobblestone] to Alice"]);
    assert_eq!(server.levels[0].pending_system_broadcasts.iter().map(|message| message.get_string()).collect::<Vec<_>>(), ["Alice has made the advancement [Stone Age]"]);
    assert!(is_done(&mut server, alice, "minecraft:story/mine_stone"));
    assert!(server.levels[0].player(alice).recipe_book.contains("minecraft:furnace"));
}

#[test]
fn saves_and_loads_progress() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    run(&mut server, "advancement grant Alice only minecraft:husbandry/balanced_diet bread");
    run(&mut server, "advancement grant Alice only minecraft:story/root");
    let path = std::env::temp_dir().join(format!("crisp-advancements-{}.json", std::process::id()));
    let manager = ServerAdvancementManager::get_bound();
    let mut saved = PlayerAdvancements::new(manager, Some(path.clone()));
    for id in ["minecraft:husbandry/balanced_diet", "minecraft:story/root"] {
        let advancement = manager.get(id).unwrap();
        for criterion in server.levels[0].player_mut(alice).advancements.get_or_start_progress(advancement).get_completed_criteria() {
            saved.get_or_start_progress(advancement).grant_progress(&criterion);
        }
    }
    saved.save();
    let text = std::fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["minecraft:husbandry/balanced_diet", "minecraft:story/root", "DataVersion"]);
    assert_eq!(json["minecraft:husbandry/balanced_diet"]["done"], false);
    assert!(text.ends_with("\n}") && text.contains("\n  \"DataVersion\": "));
    let mut loaded = PlayerAdvancements::new(manager, Some(path.clone()));
    assert!(loaded.get_or_start_progress(manager.get("minecraft:story/root").unwrap()).is_done());
    assert_eq!(loaded.get_or_start_progress(manager.get("minecraft:husbandry/balanced_diet").unwrap()).get_completed_criteria(), ["bread"]);
    std::fs::remove_file(path).unwrap();
}
