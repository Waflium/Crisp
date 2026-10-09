use crisp_core::{ChunkPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_protocol::{Reader, Writer};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::commands::arguments::selector::entity_selector_parser::EntitySelectorParser;
use crate::commands::brigadier::{self, ContextChain, StringReader};
use crate::commands::source::CommandSourceStack;
use crate::commands::{node_at, perform_prefixed_command};
use crate::network::chat::Component;
use crate::network::chat_type::ChatType;
use crate::network::connection::Outbound;
use crate::network::login::{Profile, offline_uuid};
use crate::server::Server;
use crate::server::level::game_mode::GameType;
use crate::server::level::testing;
use crate::world::difficulty::Difficulty;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;

fn server() -> Server {
    Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap()
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

fn exec(server: &mut Server, source: &CommandSourceStack, command: &str) -> Result<i32, String> {
    let units: Vec<u16> = command.encode_utf16().collect();
    let parse = brigadier::parse(StringReader::new(&units), source);
    assert!(!parse.reader.can_read(), "{command} does not parse: {:?}", parse.exceptions);
    let chain = ContextChain::try_flatten(parse.context.build(command)).expect("an executable command");
    let node = chain.executable.command.unwrap();
    let f = super::executors().into_iter().find(|(path, _)| node_at(path).id == node).expect("ported").1;
    f(server, &chain.executable, source).map_err(|e| e.message.get_string())
}

struct TestPlayer {
    id: EntityId,
    packets: UnboundedReceiver<Vec<u8>>,
}

impl TestPlayer {
    fn packets(&mut self) -> Vec<(i32, Vec<u8>)> {
        let mut out = Vec::new();
        while let Ok(packet) = self.packets.try_recv() {
            let mut r = Reader::new(&packet);
            let id = r.varint().unwrap();
            out.push((id, packet[packet.len() - r.remaining()..].to_vec()));
        }
        out
    }

    fn packet_ids(&mut self) -> Vec<i32> {
        self.packets().into_iter().map(|p| p.0).collect()
    }
}

fn join(server: &mut Server, name: &str, pos: Vec3) -> TestPlayer {
    let profile = Profile { uuid: offline_uuid(name), name: name.into(), properties: Vec::new() };
    let id = testing::player_with_profile(&mut server.level, profile, pos, 0.0, 0.0);
    let (out, packets) = Outbound::capture();
    server.level.player_mut(id).connection = out;
    server.name_to_id_cache.add(crate::server::players::NameAndId { id: offline_uuid(name), name: name.into() });
    TestPlayer { id, packets }
}

fn drop_item(server: &mut Server, pos: Vec3) -> EntityId {
    let item = ItemEntity::new(pos, ItemStack::new(crisp_data::items::STONE, 1));
    let id = item.entity.id;
    server.level.add_fresh_entity(LevelEntity::Item(item));
    id
}

fn bytes(write: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer::new();
    write(&mut w);
    w.into_inner()
}

const ORIGIN: Vec3 = Vec3::new(0.5, -60.0, 0.5);

#[test]
fn gamemode() {
    let mut server = server();
    assert_eq!(run(&mut server, "gamemode creative"), ["A player is required to run this command here"]);
    let mut alice = join(&mut server, "Alice", ORIGIN);
    assert_eq!(run(&mut server, "gamemode creative Alice"), ["Set Alice's game mode to Creative Mode"]);
    let p = server.level.player(alice.id);
    assert_eq!((p.game_mode.game_type, p.game_mode.previous_game_type), (GameType::Creative, Some(GameType::Survival)));
    let abilities = bytes(|w| {
        w.u8(1 | 4 | 8);
        w.f32(0.05);
        w.f32(0.1);
    });
    let info = bytes(|w| {
        w.u8(4);
        w.varint(1);
        w.uuid(offline_uuid("Alice"));
        w.varint(1);
    });
    let event = bytes(|w| {
        w.u8(3);
        w.f32(1.0);
    });
    let sent = alice.packets();
    assert_eq!(
        sent[..4],
        [
            (clientbound::PLAYER_ABILITIES, abilities.clone()),
            (clientbound::PLAYER_INFO_UPDATE, info),
            (clientbound::GAME_EVENT, event),
            (clientbound::PLAYER_ABILITIES, abilities),
        ]
    );
    assert_eq!(sent[4].0, clientbound::SYSTEM_CHAT);
    assert_eq!(run(&mut server, "gamemode creative Alice"), Vec::<String>::new());
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, "gamemode creative @a"), Ok(0));
    let own = CommandSourceStack::server(&server).with_entity(alice.id);
    assert_eq!(run_as(&mut server, &own, "gamemode spectator"), ["Set own game mode to Spectator Mode"]);
    assert!(server.level.player(alice.id).abilities.flying);
    assert_eq!(
        run(&mut server, "gamemode creative @e"),
        ["Only players may be affected by this command, but the provided selector includes entities", "gamemode creative @e<--[HERE]"]
    );
    assert_eq!(run(&mut server, "gamemode creative Bob"), ["No player was found"]);
}

#[test]
fn defaultgamemode() {
    let mut server = server();
    let alice = join(&mut server, "Alice", ORIGIN);
    assert_eq!(run(&mut server, "defaultgamemode adventure"), ["The default game mode is now Adventure Mode"]);
    assert_eq!(server.default_game_type, GameType::Adventure);
    assert_eq!(server.level.player(alice.id).game_mode.game_type, GameType::Survival);
    server.config.force_game_mode = true;
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, "defaultgamemode creative"), Ok(1));
    assert_eq!(server.level.player(alice.id).game_mode.game_type, GameType::Creative);
}

#[test]
fn difficulty() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    assert_eq!(run(&mut server, "difficulty"), ["The difficulty is Easy"]);
    assert_eq!(run(&mut server, "difficulty easy"), ["The difficulty did not change; it is already set to Easy"]);
    assert_eq!(run(&mut server, "difficulty hard"), ["The difficulty has been set to Hard"]);
    assert_eq!(server.difficulty, Difficulty::Hard);
    assert_eq!(alice.packets(), [(clientbound::CHANGE_DIFFICULTY, vec![3, 0])]);
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, "difficulty"), Ok(3));
    assert_eq!(exec(&mut server, &console, "difficulty peaceful"), Ok(0));
}

fn disguised(content: Component, chat_type: crate::network::chat_type::Bound) -> (i32, Vec<u8>) {
    (
        clientbound::DISGUISED_CHAT,
        bytes(|w| {
            content.write(w);
            chat_type.write(w);
        }),
    )
}

#[test]
fn say_and_me() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let bob = join(&mut server, "Bob", ORIGIN);
    assert_eq!(run(&mut server, "say hi @a!"), Vec::<String>::new());
    let names = super::list_players_command::format_list(vec![server.entity_display_name(alice.id), server.entity_display_name(bob.id)]);
    let content = Component::literal("hi ").append(names).append(Component::literal("!"));
    let bound = ChatType::SayCommand.bind(Component::literal("Server"));
    assert_eq!(bound.decorate(content.clone()).get_string(), "[Server] hi Alice, Bob!");
    assert_eq!(alice.packets(), [disguised(content, bound)]);
    run(&mut server, "say @ me");
    assert_eq!(alice.packets(), [disguised(Component::literal("@ me"), ChatType::SayCommand.bind(Component::literal("Server")))]);
    assert_eq!(run(&mut server, "say @e[foo=1]"), ["Unknown option 'foo'", "say @e[foo=1]<--[HERE]"]);
    assert_eq!(
        run(&mut server, &format!("say {}", "a".repeat(257))),
        ["Chat message was too long (257 > maximum 256 characters)"]
    );
    let mut player = CommandSourceStack::player(&server, alice.id);
    player.permission = 0;
    run_as(&mut server, &player, "me waves at @a");
    let name = server.entity_display_name(alice.id);
    assert_eq!(alice.packets(), [disguised(Component::literal("waves at @a"), ChatType::EmoteCommand.bind(name))]);
}

#[test]
fn msg() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    assert_eq!(run(&mut server, "msg Alice hello"), ["You whisper to Alice: hello"]);
    let incoming = ChatType::MsgCommandIncoming.bind(Component::literal("Server"));
    assert_eq!(incoming.decorate(Component::literal("hello")).get_string(), "Server whispers to you: hello");
    assert_eq!(alice.packets(), [disguised(Component::literal("hello"), incoming)]);
    assert_eq!(run(&mut server, "w Alice hi"), ["You whisper to Alice: hi"]);
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, "tell @a hey"), Ok(1));
    assert_eq!(run(&mut server, "tell Bob hi"), ["No player was found"]);
    let mut player = CommandSourceStack::player(&server, alice.id);
    player.permission = 0;
    alice.packets();
    run_as(&mut server, &player, "msg @a hi");
    assert_eq!(alice.packet_ids(), [clientbound::SYSTEM_CHAT, clientbound::SYSTEM_CHAT]);
    run_as(&mut server, &player, "msg Alice hi");
    let name = server.entity_display_name(alice.id);
    let outgoing = ChatType::MsgCommandOutgoing.bind(name.clone()).with_target_name(name.clone());
    let incoming = ChatType::MsgCommandIncoming.bind(name);
    assert_eq!(alice.packets(), [disguised(Component::literal("hi"), outgoing), disguised(Component::literal("hi"), incoming)]);
}

#[test]
fn kick() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let bob = join(&mut server, "Bob", ORIGIN);
    assert_eq!(run(&mut server, "kick Alice"), ["Kicked Alice: Kicked by an operator"]);
    let reason = bytes(|w| Component::translatable("multiplayer.disconnect.kicked").write(w));
    assert_eq!(alice.packets(), [(clientbound::DISCONNECT, reason)]);
    assert!(server.level.player(alice.id).disconnected);
    assert_eq!(run(&mut server, "kick Bob Go away @s"), ["Kicked Bob: Go away "]);
    assert!(server.level.player(bob.id).disconnected);
    assert_eq!(run(&mut server, "kick Carol"), ["No player was found"]);
}

#[test]
fn op_and_deop() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    assert_eq!(run(&mut server, "op alice"), ["Made Alice a server operator"]);
    let entry = server.ops.get(offline_uuid("Alice")).unwrap();
    assert_eq!((entry.user.name.as_str(), entry.level, entry.bypasses_player_limit), ("Alice", 4, false));
    let sent = alice.packets();
    let event = bytes(|w| {
        w.i32(alice.id);
        w.u8(28);
    });
    assert_eq!(sent[0], (clientbound::ENTITY_EVENT, event));
    assert_eq!(sent.iter().map(|p| p.0).collect::<Vec<_>>(), [clientbound::ENTITY_EVENT, clientbound::COMMANDS, clientbound::SYSTEM_CHAT]);
    assert_eq!(run(&mut server, "op Alice"), ["Nothing changed. The player is already an operator"]);
    assert_eq!(run(&mut server, "op Carol"), ["That player does not exist"]);
    server.name_to_id_cache.set_resolve_offline_users(true);
    assert_eq!(run(&mut server, "op Carol"), ["Made carol a server operator"]);
    assert!(server.is_op(offline_uuid("carol")));
    assert_eq!(alice.packet_ids(), [clientbound::SYSTEM_CHAT]);
    assert_eq!(run(&mut server, "deop @a"), ["Made Alice no longer a server operator"]);
    assert!(!server.is_op(offline_uuid("Alice")));
    assert_eq!(alice.packet_ids(), [clientbound::ENTITY_EVENT, clientbound::COMMANDS]);
    assert_eq!(run(&mut server, "deop Alice"), ["Nothing changed. The player is not an operator"]);
    assert_eq!(run(&mut server, "op @e"), ["Only players may be affected by this command, but the provided selector includes entities", "op @e<--[HERE]"]);
    assert_eq!(run(&mut server, "op @a[name=Bob]"), ["No player was found"]);
}

#[test]
fn teleport() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let bob = join(&mut server, "Bob", Vec3::new(20.5, -50.0, -3.25));
    assert_eq!(run(&mut server, "tp 1 2 3"), ["An entity is required to run this command here"]);
    assert_eq!(run(&mut server, "teleport Alice 10 -60 10"), ["Teleported Alice to 10.500000, -60.000000, 10.500000"]);
    assert_eq!(server.level.player(alice.id).entity.position(), Vec3::new(10.5, -60.0, 10.5));
    let position = |id: i32, relatives: i32, pos: Vec3| {
        bytes(|w| {
            w.varint(id);
            for v in [pos.x, pos.y, pos.z, 0.0, 0.0, 0.0] {
                w.f64(v);
            }
            w.f32(0.0);
            w.f32(0.0);
            w.i32(relatives);
        })
    };
    assert_eq!(alice.packets(), [(clientbound::PLAYER_POSITION, position(1, 8 | 16, Vec3::new(10.5, -60.0, 10.5)))]);
    run(&mut server, "tp Alice ~1 ~ ~-0.5");
    assert_eq!(server.level.player(alice.id).entity.position(), Vec3::new(1.0, -60.0, -0.5));
    assert_eq!(alice.packets(), [(clientbound::PLAYER_POSITION, position(2, 255, Vec3::new(-9.5, 0.0, -11.0)))]);
    assert_eq!(run(&mut server, "tp Alice Bob"), ["Teleported Alice to Bob"]);
    assert_eq!(server.level.player(alice.id).entity.position(), server.level.player(bob.id).entity.position());
    assert_eq!(run(&mut server, "tp @a Bob"), ["Teleported 2 entities to Bob"]);
    assert_eq!(run(&mut server, "tp @a 0 -60 0 90 45"), ["Teleported 2 entities to 0.500000, -60.000000, 0.500000"]);
    let a = &server.level.player(alice.id).entity;
    assert_eq!((a.y_rot, a.x_rot), (90.0, 45.0));
    assert_eq!(run(&mut server, "tp Alice 0 20000000 0"), ["Invalid position for teleport"]);
    assert_eq!(run(&mut server, "tp Alice 0 -60 0 facing 5.5 -60 0.5"), ["Teleported Alice to 0.500000, -60.000000, 0.500000"]);
    let a = &server.level.player(alice.id).entity;
    assert_eq!((a.y_rot, a.x_rot, a.y_head_rot), (-90.0, 0.0, -90.0));
    alice.packets();
    run(&mut server, "tp Alice 0 -60 0 facing entity Bob eyes");
    assert_eq!(alice.packet_ids(), [clientbound::PLAYER_POSITION, clientbound::PLAYER_LOOK_AT]);
    assert_eq!(run(&mut server, "tp @e[type=item] 0 0 0"), ["No entity was found"]);
    let item = drop_item(&mut server, ORIGIN);
    assert_eq!(run(&mut server, "tp @e[type=item] 3 -55 3"), ["Teleported Stone to 3.500000, -55.000000, 3.500000"]);
    let e = server.level.entity(item);
    assert_eq!((e.position(), e.on_ground), (Vec3::new(3.5, -55.0, 3.5), true));
    assert_eq!(run(&mut server, "tp Alice @n[type=item]"), ["Teleported Alice to Stone"]);
    assert_eq!(run(&mut server, "tp Alice @e"), ["Incorrect argument for command", "tp Alice @e<--[HERE]"]);
}

#[test]
fn kill() {
    let mut server = server();
    let alice = join(&mut server, "Alice", ORIGIN);
    assert_eq!(run(&mut server, "kill"), ["An entity is required to run this command here"]);
    assert_eq!(run(&mut server, "kill @e[type=item]"), ["No entity was found"]);
    let item = drop_item(&mut server, ORIGIN);
    assert_eq!(run(&mut server, "kill @e[type=item]"), ["Killed Stone"]);
    assert!(server.level.entity_ref(item).is_none());
    drop_item(&mut server, ORIGIN);
    drop_item(&mut server, ORIGIN);
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, "kill @e[type=!player]"), Ok(2));
    assert_eq!(run(&mut server, "kill Alice"), ["Killed Alice"]);
    assert_eq!(server.level.player(alice.id).health, 20.0);
    server.level.handle_player_loaded(alice.id);
    assert_eq!(run(&mut server, "kill Alice"), ["Killed Alice"]);
    assert_eq!(server.level.player(alice.id).health, 0.0);
    assert!(server.level.entity_ref(alice.id).is_some());
}

#[test]
fn spawnpoint() {
    let mut server = server();
    let alice = join(&mut server, "Alice", ORIGIN);
    join(&mut server, "Bob", ORIGIN);
    assert_eq!(run(&mut server, "spawnpoint"), ["A player is required to run this command here"]);
    assert_eq!(run(&mut server, "spawnpoint Alice 1 2 3"), ["Set spawn point to 1, 2, 3 [0.0] in 0.0 for minecraft:overworld"]);
    let respawn = server.level.player(alice.id).respawn_config.clone().unwrap();
    assert_eq!((respawn.respawn_data.pos, respawn.forced), (crisp_core::BlockPos::new(1, 2, 3), true));
    assert_eq!(run(&mut server, "spawnpoint @a ~ ~ ~ 270 -100"), ["Set spawn point to 0, -60, 0 [-90.0] in -90.0 for minecraft:overworld players"]);
    assert_eq!(server.level.player(alice.id).respawn_config.as_ref().unwrap().respawn_data.pitch, -90.0);
    assert_eq!(run(&mut server, "spawnpoint Alice 0 20000000 0"), ["That position is outside the allowed boundaries"]);
    let own = CommandSourceStack::server(&server).with_entity(alice.id).with_position(Vec3::new(5.7, 1.0, -0.2));
    assert_eq!(run_as(&mut server, &own, "spawnpoint"), ["Set spawn point to 5, 1, -1 [0.0] in 0.0 for minecraft:overworld"]);
}

#[test]
fn setworldspawn() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    assert_eq!(run(&mut server, "setworldspawn 5 -60 5"), ["Set the world spawn point to 5, -60, 5 [0.0]"]);
    assert_eq!(server.spawn, crisp_core::BlockPos::new(5, -60, 5));
    let spawn = bytes(|w| {
        w.string("minecraft:overworld");
        w.position(5, -60, 5);
        w.f32(0.0);
        w.f32(0.0);
    });
    assert_eq!(alice.packets(), [(clientbound::SET_DEFAULT_SPAWN_POSITION, spawn)]);
    assert_eq!(run(&mut server, "setworldspawn 5 -60 5"), ["Set the world spawn point to 5, -60, 5 [0.0]"]);
    assert!(alice.packets().is_empty());
    assert_eq!(run(&mut server, "setworldspawn 1 2 3 200 100"), ["Set the world spawn point to 1, 2, 3 [-160.0]"]);
    assert_eq!((server.spawn_yaw, server.spawn_pitch), (-160.0, 90.0));
    assert_eq!(run(&mut server, "setworldspawn 30000000 0 0"), ["That position is outside the allowed boundaries"]);
}

fn find(server: &Server, pos: Vec3, selector: &str) -> Result<Vec<EntityId>, String> {
    let units: Vec<u16> = selector.encode_utf16().collect();
    let parsed = EntitySelectorParser::new(&mut StringReader::new(&units), true).parse().unwrap();
    let source = CommandSourceStack::server(server).with_position(pos);
    parsed.find_entities(server, &source).map_err(|e| e.message.get_string())
}

#[test]
fn selectors_find_entities() {
    let mut server = server();
    let alice = join(&mut server, "Alice", Vec3::new(0.5, -60.0, 0.5)).id;
    let bob = join(&mut server, "Bob", Vec3::new(5.5, -60.0, 0.5)).id;
    let carol = join(&mut server, "Carol", Vec3::new(-2.5, -60.0, 0.5)).id;
    let near = drop_item(&mut server, Vec3::new(1.0, -60.0, 1.0));
    let far = drop_item(&mut server, Vec3::new(40.0, -60.0, 1.0));
    let find = |selector: &str| find(&server, Vec3::new(4.0, -60.0, 0.5), selector);
    assert_eq!(find("@a"), Ok(vec![alice, bob, carol]));
    assert_eq!(find("@p"), Ok(vec![bob]));
    assert_eq!(find("@a[sort=furthest]"), Ok(vec![carol, alice, bob]));
    assert_eq!(find("@a[sort=nearest,limit=2]"), Ok(vec![bob, alice]));
    assert_eq!(find("@n"), Ok(vec![bob]));
    assert_eq!(find("@n[type=item]"), Ok(vec![near]));
    assert_eq!(find("@e[type=item,sort=furthest]"), Ok(vec![far, near]));
    assert_eq!(find("@e[distance=..4]"), Ok(vec![alice, bob, near]));
    assert_eq!(find("@e[distance=3..]"), Ok(vec![alice, carol, near, far]));
    assert_eq!(find("@e[x=0,y=-61,z=0,dx=1,dy=2,dz=1]"), Ok(vec![alice, near]));
    assert_eq!(find("@e[type=!player]"), Ok(vec![near, far]));
    assert_eq!(find("@e[type=#minecraft:skeletons]"), Ok(vec![]));
    assert_eq!(find("@a[name=!Bob]"), Ok(vec![alice, carol]));
    assert_eq!(find("bob"), Ok(vec![bob]));
    assert_eq!(find(&crate::world::level::storage::format_uuid(offline_uuid("Carol"))), Ok(vec![carol]));
    assert_eq!(find("@e[tag=]").map(|e| e.len()), Ok(5));
    assert_eq!(find("@e[tag=x]"), Ok(vec![]));
    assert_eq!(find("@e[team=!]"), Ok(vec![]));
    assert_eq!(find("@a[level=0]").map(|e| e.len()), Ok(3));
    assert_eq!(find("@a[level=1..]"), Ok(vec![]));
    assert_eq!(find("@e[scores={kills=1..}]"), Ok(vec![]));
    assert_eq!(find("@a[gamemode=survival,limit=1]"), Ok(vec![alice]));
    assert_eq!(find("@a[x_rotation=0]").map(|e| e.len()), Ok(3));
    assert_eq!(find("@a[y_rotation=10..20]"), Ok(vec![]));
    assert_eq!(find("@s"), Ok(vec![]));
    assert_eq!(find("@a[advancements={story/root=true}]"), Err("An unexpected error occurred while trying to execute that command".into()));
    let random = find("@r").unwrap();
    assert!(random.len() == 1 && [alice, bob, carol].contains(&random[0]));
    let mut shuffled = find("@r[limit=5]").unwrap();
    shuffled.sort();
    assert_eq!(shuffled, [alice, bob, carol]);
    let units: Vec<u16> = "@a".encode_utf16().collect();
    let selector = EntitySelectorParser::new(&mut StringReader::new(&units), true).parse().unwrap();
    let mut player = CommandSourceStack::player(&server, alice);
    player.permission = 0;
    assert_eq!(selector.find_players(&server, &player).map_err(|e| e.message.get_string()), Err("Selector not allowed".into()));
}

#[test]
fn tellraw_resolves_scores_for_each_player() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let mut bob = join(&mut server, "Bob", ORIGIN);
    run(&mut server, "scoreboard objectives add o dummy");
    run(&mut server, "scoreboard players set Alice o 5");
    alice.packets();
    bob.packets();
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, r#"tellraw @a {"score":{"name":"*","objective":"o"}}"#), Ok(2));
    let chat = |c: Component| {
        (
            clientbound::SYSTEM_CHAT,
            bytes(|w| {
                c.write(w);
                w.bool(false);
            }),
        )
    };
    assert_eq!(alice.packets(), [chat(Component::literal("5"))]);
    assert_eq!(bob.packets(), [chat(Component::empty())]);
}

#[test]
fn title() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let mut bob = join(&mut server, "Bob", ORIGIN);
    assert_eq!(run(&mut server, "title Alice title \"Hi\""), ["Showing new title for Alice"]);
    let text = bytes(|w| Component::literal("Hi").write(w));
    assert_eq!(alice.packets(), [(clientbound::SET_TITLE_TEXT, text.clone())]);
    assert_eq!(run(&mut server, "title @a subtitle \"Hi\""), ["Showing new subtitle for 2 players"]);
    assert_eq!(bob.packets(), [(clientbound::SET_SUBTITLE_TEXT, text.clone())]);
    assert_eq!(run(&mut server, "title @a actionbar \"Hi\""), ["Showing new actionbar title for 2 players"]);
    assert_eq!(bob.packets(), [(clientbound::SET_ACTION_BAR_TEXT, text)]);
    assert_eq!(run(&mut server, "title Bob times 1s 2 3t"), ["Changed title display times for Bob"]);
    let times = bytes(|w| {
        w.i32(20);
        w.i32(2);
        w.i32(3);
    });
    assert_eq!(bob.packets(), [(clientbound::SET_TITLES_ANIMATION, times)]);
    assert_eq!(run(&mut server, "title Bob clear"), ["Cleared titles for Bob"]);
    assert_eq!(run(&mut server, "title Bob reset"), ["Reset title options for Bob"]);
    assert_eq!(bob.packets(), [(clientbound::CLEAR_TITLES, vec![0]), (clientbound::CLEAR_TITLES, vec![1])]);
    alice.packets();
}

#[test]
fn tag() {
    let mut server = server();
    let alice = join(&mut server, "Alice", ORIGIN);
    let item = drop_item(&mut server, ORIGIN);
    assert_eq!(run(&mut server, "tag Alice list"), ["There are no tags on the 0 entities"]);
    assert_eq!(run(&mut server, "tag @e list"), ["There are no tags on the 0 entities"]);
    assert_eq!(run(&mut server, "tag Alice add b"), ["Added tag 'b' to Alice"]);
    assert_eq!(run(&mut server, "tag @e add a"), ["Added tag 'a' to 2 entities"]);
    assert_eq!(run(&mut server, "tag @e add b"), ["Added tag 'b' to Stone"]);
    assert_eq!(run(&mut server, "tag Alice add b"), ["Target either already has the tag or has too many tags"]);
    assert_eq!(server.level.entity(alice.id).tags, ["b", "a"]);
    assert_eq!(run(&mut server, "tag Alice list"), ["Alice has 2 tag(s): a, b"]);
    assert_eq!(run(&mut server, "tag @e list"), ["The 2 entities have 2 total tag(s): a, b"]);
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, "tag @e list"), Ok(2));
    assert_eq!(exec(&mut server, &console, "tag @e remove a"), Ok(2));
    assert_eq!(run(&mut server, "tag @e remove a"), ["Target does not have this tag"]);
    assert_eq!(server.level.entity(item).tags, ["b"]);
    assert_eq!(run(&mut server, "tag @e[tag=b] list"), ["The 2 entities have 1 total tag(s): b"]);
    let saved = server.level.player(alice.id).to_nbt("minecraft:overworld");
    let bob = join(&mut server, "Bob", ORIGIN);
    server.level.player_mut(bob.id).load_nbt(&saved);
    assert_eq!(server.level.entity(bob.id).tags, ["b"]);
}

#[test]
fn weather() {
    let mut server = server();
    let console = CommandSourceStack::server(&server);
    assert_eq!(run(&mut server, "weather thunder 10s"), ["Set the weather to rain and thunder"]);
    let w = server.level.weather;
    assert_eq!((w.clear_weather_time, w.rain_time, w.thunder_time, w.raining, w.thundering), (0, 200, 200, true, true));
    assert_eq!(exec(&mut server, &console, "weather rain"), Ok(-1));
    let w = server.level.weather;
    assert!((12000..=24000).contains(&w.rain_time) && w.raining && !w.thundering);
    assert_eq!(exec(&mut server, &console, "weather clear 5"), Ok(5));
    let w = server.level.weather;
    assert_eq!((w.clear_weather_time, w.rain_time, w.thunder_time, w.raining, w.thundering), (5, 0, 0, false, false));
}

#[test]
fn rotate() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    alice.packets();
    assert_eq!(run(&mut server, "rotate Alice 90 100"), ["Rotated Alice"]);
    let e = server.level.entity(alice.id);
    assert_eq!((e.y_rot, e.x_rot), (90.0, 90.0));
    let rotation = |y: f32, ry: bool, x: f32, rx: bool| {
        (
            clientbound::PLAYER_ROTATION,
            bytes(|w| {
                w.f32(y);
                w.bool(ry);
                w.f32(x);
                w.bool(rx);
            }),
        )
    };
    assert_eq!(alice.packets(), [rotation(90.0, false, 100.0, false)]);
    run(&mut server, "rotate Alice ~10 ~");
    let e = server.level.entity(alice.id);
    assert_eq!((e.y_rot, e.x_rot), (10.0, 0.0));
    assert_eq!(alice.packets(), [rotation(-80.0, true, -90.0, true)]);
    let item = drop_item(&mut server, Vec3::new(0.5, -60.0, 5.5));
    assert_eq!(run(&mut server, "rotate @e[type=item,limit=1] facing 0.5 -60 10.5"), ["Rotated Stone"]);
    assert_eq!(server.level.entity(item).y_rot, 0.0);
}

#[test]
fn playsound_and_stopsound() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let mut bob = join(&mut server, "Bob", Vec3::new(100.5, -60.0, 0.5));
    alice.packets();
    bob.packets();
    let console = CommandSourceStack::server(&server);
    assert_eq!(run(&mut server, "playsound minecraft:block.note_block.bass"), ["The sound is too far away to be heard"]);
    assert_eq!(run(&mut server, "playsound block.note_block.bass ambient @a 0.5 -60 0.5"), ["Played sound minecraft:block.note_block.bass to Alice"]);
    assert!(bob.packets().is_empty());
    let sound = alice.packets();
    assert_eq!(sound.len(), 1);
    assert_eq!(sound[0].0, clientbound::SOUND);
    let mut r = crisp_protocol::Reader::new(&sound[0].1);
    assert_eq!((r.varint().unwrap(), r.string(32767).unwrap(), r.bool().unwrap(), r.varint().unwrap()), (0, "minecraft:block.note_block.bass", false, 8));
    assert_eq!((r.i32().unwrap(), r.i32().unwrap(), r.i32().unwrap(), r.f32().unwrap(), r.f32().unwrap()), (4, -480, 4, 1.0, 1.0));
    assert_eq!(exec(&mut server, &console, "playsound block.note_block.bass master @a 0.5 -60 0.5 1 1 0.2"), Ok(2));
    let far = bob.packets();
    let mut r = crisp_protocol::Reader::new(&far[0].1);
    r.varint().unwrap();
    r.string(32767).unwrap();
    r.bool().unwrap();
    r.varint().unwrap();
    assert_eq!((r.i32().unwrap(), r.i32().unwrap(), r.i32().unwrap(), r.f32().unwrap()), (788, -480, 4, 0.2));
    alice.packets();
    assert_eq!(run(&mut server, "stopsound @a"), ["Stopped all sounds"]);
    assert_eq!(run(&mut server, "stopsound @a * ui.button.click"), ["Stopped sound 'minecraft:ui.button.click'"]);
    assert_eq!(run(&mut server, "stopsound Bob weather"), ["Stopped all 'weather' sounds"]);
    assert_eq!(run(&mut server, "stopsound Bob player a"), ["Stopped sound 'minecraft:a' on source 'player'"]);
    assert_eq!(alice.packet_ids(), [clientbound::STOP_SOUND, clientbound::STOP_SOUND]);
    let stops: Vec<Vec<u8>> = bob.packets().into_iter().map(|p| p.1).collect();
    assert_eq!(stops[0], [0]);
    assert_eq!(stops[2], [1, 3]);
    assert_eq!(stops[3][..2], [3, 7]);
}

#[test]
fn teammsg() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let mut bob = join(&mut server, "Bob", ORIGIN);
    let mut carol = join(&mut server, "Carol", ORIGIN);
    let as_alice = CommandSourceStack::player(&server, alice.id);
    assert_eq!(exec(&mut server, &as_alice, "teammsg hi"), Err("You must be on a team to message your team".into()));
    run(&mut server, "team add red");
    run(&mut server, "team join red Alice");
    run(&mut server, "team join red Bob");
    for p in [&mut alice, &mut bob, &mut carol] {
        p.packets();
    }
    let as_alice = CommandSourceStack::player(&server, alice.id);
    assert_eq!(exec(&mut server, &as_alice, "tm hi"), Ok(2));
    let name = server.entity_display_name(alice.id);
    let team = server.level.team_of(alice.id).unwrap().formatted_display_name();
    let team = team
        .with_hover(crate::network::chat::HoverEvent::ShowText(Box::new(Component::translatable("chat.type.team.hover"))))
        .with_click(crate::network::chat::ClickEvent::SuggestCommand("/teammsg ".into()));
    let outgoing = ChatType::TeamMsgCommandOutgoing.bind(name.clone()).with_target_name(team.clone());
    let incoming = ChatType::TeamMsgCommandIncoming.bind(name).with_target_name(team);
    assert_eq!(outgoing.decorate(Component::literal("hi")).get_string(), "-> [red] <Alice> hi");
    assert_eq!(alice.packets(), [disguised(Component::literal("hi"), outgoing)]);
    assert_eq!(bob.packets(), [disguised(Component::literal("hi"), incoming)]);
    assert!(carol.packets().is_empty());
}

#[test]
fn random() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let console = CommandSourceStack::server(&server);
    assert_eq!(run(&mut server, "random value 5..5"), ["The range of the random value must be at least 1"]);
    assert_eq!(run(&mut server, "random value ..0"), ["The range of the random value must be at most 2147483647"]);
    let value = exec(&mut server, &console, "random value 1..6").unwrap();
    assert!((1..=6).contains(&value));
    let first = exec(&mut server, &console, "random value 1..1000 minecraft:dice").unwrap();
    assert_eq!(run(&mut server, "random reset minecraft:dice"), ["Reset random sequence minecraft:dice"]);
    assert_eq!(exec(&mut server, &console, "random value 1..1000 minecraft:dice"), Ok(first));
    assert_eq!(run(&mut server, "random reset * 3 false"), ["Reset 1 random sequence(s)"]);
    alice.packets();
    let rolled = run(&mut server, "random roll 1..6");
    assert!(rolled[0].starts_with("Server rolled "), "{rolled:?}");
    assert_eq!(alice.packet_ids(), [clientbound::SYSTEM_CHAT]);
}

#[test]
fn version() {
    let mut server = server();
    let lines = run(&mut server, "version");
    assert_eq!(lines[..6], ["Server version info:", "id = 26.3", "name = 26.3", "data = 5023", "series = main", "protocol = 777 (0x309)"]);
    assert!(lines[6].starts_with("build_time = Tue Sep 15 ") || lines[6].starts_with("build_time = "));
    assert_eq!(lines[7..], ["pack_resource = 97.1", "pack_data = 121.0", "stable = yes"]);
}

#[test]
fn data_on_entities() {
    let mut server = server();
    let alice = join(&mut server, "Alice", ORIGIN);
    let item = drop_item(&mut server, ORIGIN);
    let uuid = server.level.entity(item).uuid;
    assert_eq!(run(&mut server, "data get entity @e[type=item,limit=1] Item.count"), ["Stone has the following entity data: 1"]);
    assert_eq!(run(&mut server, "data merge entity @e[type=item,limit=1] {Item:{id:\"minecraft:dirt\",count:5},Tags:[\"x\"],UUID:[I;1,2,3,4]}"), ["Modified entity data of Dirt"]);
    let LevelEntity::Item(loaded) = &server.level.entities[&item] else { panic!("not an item") };
    assert_eq!((loaded.item.item, loaded.item.count), (crisp_data::items::DIRT, 5));
    assert_eq!((loaded.entity.uuid, loaded.entity.tags.clone()), (uuid, vec!["x".to_string()]));
    assert_eq!(run(&mut server, "data get entity @e[type=item,limit=1] Item.count 2"), ["Item.count on Dirt after scale factor of 2.00 is 10"]);
    assert_eq!(run(&mut server, "data merge entity Alice {Invulnerable:1b}"), ["Unable to modify player data"]);
    server.level.player_mut(alice.id).inventory.slots[36] = Some(ItemStack::new(crisp_data::items::STONE, 1));
    assert_eq!(find(&server, ORIGIN, "@a[nbt={SelectedItem:{id:\"minecraft:stone\"}}]"), Ok(vec![alice.id]));
    assert_eq!(find(&server, ORIGIN, "@e[nbt=!{Tags:[\"x\"]}]"), Ok(vec![alice.id]));
    assert_eq!(find(&server, ORIGIN, "@e[nbt={Tags:[\"x\"]}]"), Ok(vec![item]));
}

#[test]
fn bossbar() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let console = CommandSourceStack::server(&server);
    assert_eq!(run(&mut server, "bossbar list"), ["There are no custom bossbars active"]);
    assert_eq!(exec(&mut server, &console, "bossbar add test \"Boss\""), Ok(1));
    assert_eq!(run(&mut server, "bossbar add test \"Boss\""), ["A bossbar already exists with the ID 'minecraft:test'"]);
    assert_eq!(run(&mut server, "bossbar list"), ["There are 1 custom bossbar(s) active: [Boss]"]);
    assert_eq!(run(&mut server, "bossbar get nope value"), ["No bossbar exists with the ID 'minecraft:nope'"]);
    alice.packets();
    assert_eq!(run(&mut server, "bossbar set test players Alice"), ["Custom bossbar [Boss] now has 1 player(s): Alice"]);
    let id = server.custom_boss_events.get("minecraft:test").unwrap().event.id;
    let add = bytes(|w| {
        w.uuid(id);
        w.varint(0);
        Component::literal("Boss").write(w);
        w.f32(0.0);
        w.varint(6);
        w.varint(0);
        w.u8(0);
    });
    assert_eq!(alice.packets(), [(clientbound::BOSS_EVENT, add)]);
    assert_eq!(run(&mut server, "bossbar set test players Alice"), ["Nothing changed. Those players are already on the bossbar with nobody to add or remove"]);
    assert_eq!(exec(&mut server, &console, "bossbar set test value 50"), Ok(50));
    let progress = bytes(|w| {
        w.uuid(id);
        w.varint(2);
        w.f32(0.5);
    });
    assert_eq!(alice.packets(), [(clientbound::BOSS_EVENT, progress)]);
    assert_eq!(run(&mut server, "bossbar set test value 50"), ["Nothing changed. That's already the value of this bossbar"]);
    assert_eq!(run(&mut server, "bossbar set test color red"), ["Custom bossbar [Boss] has changed color"]);
    assert_eq!(run(&mut server, "bossbar set test style notched_10"), ["Custom bossbar [Boss] has changed style"]);
    assert_eq!(alice.packet_ids(), [clientbound::BOSS_EVENT, clientbound::BOSS_EVENT]);
    assert_eq!(run(&mut server, "bossbar get test players"), ["Custom bossbar [Boss] has 1 player(s) currently online: Alice"]);
    assert_eq!(exec(&mut server, &console, "bossbar get test visible"), Ok(1));
    assert_eq!(run(&mut server, "bossbar set test visible false"), ["Custom bossbar [Boss] is now hidden"]);
    assert_eq!(alice.packets()[0].1[16], 1);
    assert_eq!(run(&mut server, "bossbar set test name \"Big\""), ["Custom bossbar [Big] has been renamed"]);
    assert!(alice.packets().is_empty());
    let packed = server.custom_boss_events.get("minecraft:test").unwrap().pack();
    assert_eq!(packed.get("Color"), Some(&crisp_protocol::nbt::Nbt::String("red".into())));
    assert_eq!(packed.get("Visible"), None);
    assert_eq!(exec(&mut server, &console, "bossbar remove test"), Ok(0));
    assert_eq!(run(&mut server, "bossbar list"), ["There are no custom bossbars active"]);
}

#[test]
fn forceload() {
    let dir = std::env::temp_dir().join(format!("crisp-forceload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = || crate::ServerConfig { level_type: crate::LevelType::Flat, world: Some(dir.clone()), ..crate::ServerConfig::default() };
    let mut server = Server::new(config()).unwrap();
    let console = CommandSourceStack::server(&server);
    assert_eq!(run(&mut server, "forceload query"), ["No force loaded chunks were found in minecraft:overworld"]);
    assert_eq!(run(&mut server, "forceload add 100 100"), ["Marked chunk [6, 6] in minecraft:overworld to be force loaded"]);
    server.level.chunks.run_until_idle();
    assert!(server.level.chunks.is_position_ticking(ChunkPos::new(6, 6)));
    assert_eq!(run(&mut server, "forceload add 100 100"), ["No chunks were marked for force loading"]);
    assert_eq!(run(&mut server, "forceload add 90 90 130 100"), ["Marked 7 chunks in minecraft:overworld from [5, 5] to [8, 6] to be force loaded"]);
    assert_eq!(run(&mut server, "forceload query"), ["8 force loaded chunks were found in minecraft:overworld at: [5, 5], [6, 5], [7, 5], [8, 5], [5, 6], [6, 6], [7, 6], [8, 6]"]);
    assert_eq!(run(&mut server, "forceload query 100 100"), ["Chunk at [6, 6] in minecraft:overworld is marked for force loading"]);
    assert_eq!(run(&mut server, "forceload query 0 0"), ["Chunk at [0, 0] in minecraft:overworld is not marked for force loading"]);
    assert_eq!(run(&mut server, "forceload add 0 0 300 300"), ["Too many chunks in the specified area (maximum 256, but specified 361)"]);
    assert_eq!(exec(&mut server, &console, "forceload remove 80 80"), Ok(1));
    server.save_everything(true, true, true);
    drop(server);
    let mut server = Server::new(config()).unwrap();
    assert_eq!(server.level.chunks.force_loaded_chunks().len(), 7);
    assert_eq!(run(&mut server, "forceload remove all"), ["Unmarked all force loaded chunks in minecraft:overworld"]);
    assert!(server.level.chunks.force_loaded_chunks().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tick() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let item = drop_item(&mut server, Vec3::new(0.5, -50.0, 0.5));
    server.level.chunks.run_until_idle();
    server.level.process_chunk_events();
    server.tick();
    alice.packets();
    assert_eq!(run(&mut server, "tick step"), ["Unable to step the game - the game must be frozen first"]);
    assert_eq!(run(&mut server, "tick freeze"), ["The game is frozen"]);
    let state = bytes(|w| {
        w.f32(20.0);
        w.bool(true);
    });
    assert_eq!(alice.packets(), [(clientbound::TICKING_STATE, state)]);
    let (time, y) = (server.level.game_time, server.level.entity(item).y());
    for _ in 0..5 {
        server.tick();
    }
    assert_eq!((server.level.game_time, server.level.entity(item).y()), (time, y));
    assert_eq!(run(&mut server, "tick step 2"), ["Stepping 2 tick(s)"]);
    for _ in 0..5 {
        server.tick();
    }
    assert_eq!(server.level.game_time, time + 2);
    assert_ne!(server.level.entity(item).y(), y);
    assert_eq!(run(&mut server, "tick step stop"), ["No tick step in progress"]);
    assert_eq!(run(&mut server, "tick rate 40"), ["Set the target tick rate to 40.0 per second"]);
    assert_eq!(server.level.tick_rate_manager.nanoseconds_per_tick(), 25_000_000);
    let query = run(&mut server, "tick query");
    assert_eq!(query[0], "The game is frozen");
    assert!(query[1].starts_with("Target tick rate: 40.0 per second.\nAverage time per tick: "), "{query:?}");
    assert!(query[2].starts_with("Percentiles: P50: ") && query[2].ends_with("Sample: 100"), "{query:?}");
    assert_eq!(run(&mut server, "tick unfreeze"), ["The game is running normally"]);
    assert_eq!(run(&mut server, "tick sprint stop"), ["No tick sprint in progress"]);
    assert_eq!(run(&mut server, "tick sprint 3"), ["The game is sprinting"]);
    assert!(!server.check_should_sprint_this_tick());
    server.tick();
    for _ in 0..3 {
        assert!(server.check_should_sprint_this_tick());
        server.tick();
    }
    server.console_log.borrow_mut().clear();
    assert!(!server.check_should_sprint_this_tick());
    assert!(server.console_log.borrow()[0].starts_with("Sprint completed with "));
    assert!(!server.tick_rate_manager.is_sprinting() && !server.level.tick_rate_manager.is_frozen());
}

#[test]
fn swing() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let mut bob = join(&mut server, "Bob", Vec3::new(2.5, -60.0, 0.5));
    let item = drop_item(&mut server, Vec3::new(4.5, -60.0, 6.5));
    server.level.chunks.run_until_idle();
    server.level.handle_player_loaded(alice.id);
    server.level.handle_player_loaded(bob.id);
    server.level.finish_chunk_work();
    for _ in 0..10 {
        for id in [alice.id, bob.id] {
            server.level.send_next_chunks(id);
            server.level.player_mut(id).chunk_sender.on_chunk_batch_received(64.0);
        }
    }
    server.tick();
    run(&mut server, "tp Bob 17.5 -60 0.5");
    server.tick();
    alice.packets();
    bob.packets();
    let swing = |id: EntityId, hand: i32, ty: i32, duration: i32| {
        (
            clientbound::SWING_ANIMATION,
            bytes(|w| {
                w.varint(id);
                w.varint(hand);
                w.varint(ty);
                w.varint(duration);
            }),
        )
    };
    let punch = crate::network::connection::Packet { id: crisp_data::packets::play::serverbound::PUNCH, data: Vec::new() };
    server.level.handle_game_packet(alice.id, &punch).unwrap();
    assert!(alice.packets().is_empty());
    assert_eq!(bob.packets(), [swing(alice.id, 0, 1, 6)]);
    server.tick();
    bob.packets();
    server.level.handle_game_packet(alice.id, &punch).unwrap();
    assert!(!bob.packet_ids().contains(&clientbound::SWING_ANIMATION));
    assert_eq!(run(&mut server, "swing Bob offhand stab 10t"), ["Made Bob swing an arm"]);
    let swings = |p: &mut TestPlayer| p.packets().into_iter().filter(|p| p.0 == clientbound::SWING_ANIMATION).collect::<Vec<_>>();
    assert_eq!(swings(&mut bob), [swing(bob.id, 1, 2, 10)]);
    assert_eq!(swings(&mut alice), [swing(bob.id, 1, 2, 10)]);
    let console = CommandSourceStack::server(&server);
    assert_eq!(exec(&mut server, &console, &format!("swing {}", "@e[type=item]")), Err("No living entities were found to swing".into()));
    let _ = item;
}

#[test]
fn stopwatch_transfer_and_damage() {
    let mut server = server();
    let mut alice = join(&mut server, "Alice", ORIGIN);
    let console = CommandSourceStack::server(&server);
    assert_eq!(run(&mut server, "stopwatch create race"), ["Created stopwatch 'minecraft:race'"]);
    assert_eq!(run(&mut server, "stopwatch create race"), ["Stopwatch 'minecraft:race' already exists"]);
    assert!(exec(&mut server, &console, "stopwatch query race 1000").unwrap() >= 0);
    assert!(run(&mut server, "stopwatch query race")[0].starts_with("Stopwatch 'minecraft:race' has run for "));
    assert_eq!(run(&mut server, "stopwatch restart race"), ["Restarted stopwatch 'minecraft:race'"]);
    assert_eq!(run(&mut server, "stopwatch remove race"), ["Removed stopwatch 'minecraft:race'"]);
    assert_eq!(run(&mut server, "stopwatch remove race"), ["Stopwatch 'minecraft:race' does not exist"]);

    alice.packets();
    assert_eq!(run(&mut server, "transfer example.org 25566 Alice"), ["Transferring Alice to example.org:25566"]);
    let transfer = bytes(|w| {
        w.string("example.org");
        w.varint(25566);
    });
    assert_eq!(alice.packets(), [(clientbound::TRANSFER, transfer)]);
    assert_eq!(run(&mut server, "transfer example.org"), ["A player is required to run this command here"]);

    assert_eq!(run(&mut server, "damage Alice 3"), ["Target is invulnerable to the given damage type"]);
    server.level.handle_player_loaded(alice.id);
    assert_eq!(run(&mut server, "damage Alice 3"), ["Applied 3.0 damage to Alice"]);
    assert_eq!(server.level.player(alice.id).health, 17.0);
    server.level.player_mut(alice.id).abilities.invulnerable = true;
    assert_eq!(run(&mut server, "damage Alice 3 minecraft:mob_attack by Alice"), ["Target is invulnerable to the given damage type"]);
    assert_eq!(exec(&mut server, &console, "damage Alice 5 minecraft:out_of_world at 0 0 0"), Ok(1));
}
