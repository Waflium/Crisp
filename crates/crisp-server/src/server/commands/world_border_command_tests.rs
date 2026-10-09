use crisp_data::packets::play::clientbound;
use crisp_protocol::{Reader, Writer};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::commands::brigadier::{self, ContextChain, StringReader};
use crate::commands::source::CommandSourceStack;
use crate::commands::{node_at, perform_prefixed_command};
use crate::network::configuration::CommonListenerCookie;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::server::Server;

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

fn packets(rx: &mut UnboundedReceiver<Vec<u8>>) -> Vec<(i32, Vec<u8>)> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .map(|packet| {
            let mut r = Reader::new(&packet);
            let id = r.varint().unwrap();
            (id, packet[packet.len() - r.remaining()..].to_vec())
        })
        .collect()
}

fn bytes(write: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer::new();
    write(&mut w);
    w.into_inner()
}

fn join(server: &mut Server, name: &str) -> UnboundedReceiver<Vec<u8>> {
    let (out, rx) = Outbound::capture();
    let profile = Profile { uuid: 7, name: name.into(), properties: Vec::new() };
    server.place_new_player(1, profile, CommonListenerCookie::create_initial(false), out, "127.0.0.1:25565".parse().unwrap());
    rx
}

#[test]
fn size() {
    let mut server = server();
    assert_eq!(run(&mut server, "worldborder get"), ["The world border is currently 59999968 block(s) wide"]);
    assert_eq!(exec(&mut server, "worldborder get"), Ok(59_999_968));
    assert_eq!(run(&mut server, "worldborder set 100"), ["Set the world border to 100.0 block(s) wide"]);
    assert_eq!(exec(&mut server, "worldborder set 100.25"), Ok(0));
    assert_eq!(exec(&mut server, "worldborder set 100"), Ok(0));
    assert_eq!(run(&mut server, "worldborder set 100"), ["Nothing changed. The world border is already that size"]);
    assert_eq!(run(&mut server, "worldborder set 0.5"), ["World border cannot be smaller than 1 block wide"]);
    assert_eq!(run(&mut server, "worldborder set 59999970"), ["Double must not be more than 5.9999968E7: found 5.999997E7", "...order set 59999970<--[HERE]"]);
    assert_eq!(run(&mut server, "worldborder set 50 10"), ["Shrinking the world border to 50.0 block(s) wide over 0.50 second(s)"]);
    for _ in 0..4 {
        server.level.tick();
    }
    assert_eq!(run(&mut server, "worldborder get"), ["The world border is currently 80 block(s) wide"]);
    assert_eq!(run(&mut server, "worldborder add 10 5"), ["Growing the world border to 90.0 block(s) wide over 0.55 second(s)"]);
    assert_eq!((server.level.world_border.get_lerp_time(), server.level.world_border.get_lerp_target()), (11, 90.0));
    assert_eq!(exec(&mut server, "worldborder add -40.5"), Ok(-40));
    assert_eq!(server.level.world_border.get_lerp_time(), 0);
    assert_eq!(exec(&mut server, "worldborder get"), Ok(40));
    run(&mut server, "worldborder set 59999960");
    assert_eq!(run(&mut server, "worldborder add 10"), ["World border cannot be bigger than 5.999997E7 blocks wide"]);
    assert_eq!(exec(&mut server, "worldborder add -59999959"), Ok(-59_999_959));
    assert_eq!(run(&mut server, "worldborder get"), ["The world border is currently 1 block(s) wide"]);
    assert_eq!(exec(&mut server, "worldborder get"), Ok(1));
}

#[test]
fn center() {
    let mut server = server();
    assert_eq!(run(&mut server, "worldborder center 10.7 -3.2"), ["Set the center of the world border to 10.70, -3.20"]);
    let border = &server.level.world_border;
    assert_eq!((border.get_center_x(), border.get_center_z()), (10.7f32 as f64, -3.2f32 as f64));
    assert_eq!(run(&mut server, "worldborder center 10.7 -3.2"), ["Nothing changed. The world border is already centered there"]);
    assert_eq!(exec(&mut server, "worldborder center 5 5"), Ok(0));
    assert_eq!((server.level.world_border.get_center_x(), server.level.world_border.get_center_z()), (5.5, 5.5));
    assert_eq!(run(&mut server, "worldborder center 30000000 0"), ["World border cannot be further out than 2.9999984E7 blocks"]);
    assert_eq!(run(&mut server, "worldborder center 29999984 -29999984"), ["Set the center of the world border to 29999984.00, -29999984.00"]);
}

#[test]
fn damage_and_warnings() {
    let mut server = server();
    assert_eq!(run(&mut server, "worldborder damage amount 0.2"), ["Set the world border damage to 0.20 per block each second"]);
    assert_eq!(server.level.world_border.get_damage_per_block(), 0.2f32 as f64);
    assert_eq!(run(&mut server, "worldborder damage amount 0.2"), ["Nothing changed. The world border damage is already that amount"]);
    assert_eq!(exec(&mut server, "worldborder damage amount 2.5"), Ok(2));
    assert_eq!(run(&mut server, "worldborder damage buffer 5"), ["Nothing changed. The world border damage buffer is already that distance"]);
    assert_eq!(run(&mut server, "worldborder damage buffer 7.125"), ["Set the world border damage buffer to 7.13 block(s)"]);
    assert_eq!(exec(&mut server, "worldborder damage buffer 9.9"), Ok(9));
    assert_eq!(run(&mut server, "worldborder warning distance 5"), ["Nothing changed. The world border warning is already that distance"]);
    assert_eq!(run(&mut server, "worldborder warning distance 12"), ["Set the world border warning distance to 12 block(s)"]);
    assert_eq!(exec(&mut server, "worldborder warning distance 13"), Ok(13));
    assert_eq!(run(&mut server, "worldborder warning time 15s"), ["Nothing changed. The world border warning is already that amount of time"]);
    assert_eq!(run(&mut server, "worldborder warning time 15"), ["Set the world border warning time to 0.75 second(s)"]);
    assert_eq!(exec(&mut server, "worldborder warning time 1d"), Ok(24000));
    assert_eq!(server.level.world_border.get_warning_time(), 24000);
}

#[test]
fn players_are_told() {
    let mut server = server();
    run(&mut server, "worldborder center 3 -4");
    run(&mut server, "worldborder set 100 40");
    run(&mut server, "worldborder warning time 20");
    server.level.tick();
    let mut rx = join(&mut server, "Alice");
    let joined = packets(&mut rx);
    let initialize = joined.iter().find(|p| p.0 == clientbound::INITIALIZE_BORDER).expect("the border");
    let size = 59_999_968.0 + (1.0 / 40.0) * (100.0 - 59_999_968.0);
    let expected = bytes(|w| {
        w.f64(3.5);
        w.f64(-3.5);
        w.f64(size);
        w.f64(100.0);
        w.varlong(39);
        w.varint(29_999_984);
        w.varint(5);
        w.varint(20);
    });
    assert_eq!(initialize.1, expected);
    run(&mut server, "worldborder set 64");
    run(&mut server, "worldborder add 16 20");
    run(&mut server, "worldborder center 0 0");
    run(&mut server, "worldborder warning distance 9");
    run(&mut server, "worldborder warning time 30");
    run(&mut server, "worldborder damage amount 3");
    run(&mut server, "worldborder damage buffer 1");
    let sent: Vec<(i32, Vec<u8>)> = packets(&mut rx).into_iter().filter(|p| p.0 != clientbound::SYSTEM_CHAT).collect();
    assert_eq!(
        sent,
        [
            (clientbound::SET_BORDER_SIZE, bytes(|w| w.f64(64.0))),
            (
                clientbound::SET_BORDER_LERP_SIZE,
                bytes(|w| {
                    w.f64(64.0);
                    w.f64(80.0);
                    w.varlong(20);
                })
            ),
            (
                clientbound::SET_BORDER_CENTER,
                bytes(|w| {
                    w.f64(0.5);
                    w.f64(0.5);
                })
            ),
            (clientbound::SET_BORDER_WARNING_DISTANCE, bytes(|w| w.varint(9))),
            (clientbound::SET_BORDER_WARNING_DELAY, bytes(|w| w.varint(30))),
        ]
    );
}

#[test]
fn the_spawn_stays_inside_the_border() {
    let mut server = server();
    run(&mut server, "worldborder center 100 100");
    run(&mut server, "worldborder set 10");
    assert_eq!(server.respawn_data().pos, crisp_core::BlockPos::new(0, -60, 0));
    server.tick();
    let moved = crisp_core::BlockPos::new(100, -60, 100);
    assert_eq!(server.respawn_data().pos, moved);
    assert_eq!(server.spawn, crisp_core::BlockPos::new(0, -60, 0), "the world spawn itself stays");
    let mut rx = join(&mut server, "Alice");
    let spawn = packets(&mut rx).into_iter().find(|p| p.0 == clientbound::SET_DEFAULT_SPAWN_POSITION).unwrap();
    let mut r = Reader::new(&spawn.1);
    assert_eq!(r.string(32767).unwrap(), "minecraft:overworld");
    assert_eq!(r.position().unwrap(), (100, -60, 100));
    let player = server.connections[&1];
    let joined = server.level.entity(player).block_position();
    assert!(joined.x.abs() <= 1 && joined.z.abs() <= 1 && joined.y == -60, "new players search around the world spawn itself: {joined:?}");
}

#[test]
fn saved_with_the_overworld() {
    let dir = std::env::temp_dir().join(format!("crisp-worldborder-command-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = crate::ServerConfig { level_type: crate::LevelType::Flat, world: Some(dir.clone()), ..crate::ServerConfig::default() };
    let mut first = Server::new(config.clone()).unwrap();
    run(&mut first, "worldborder set 1000 100");
    run(&mut first, "worldborder damage buffer 2");
    for _ in 0..25 {
        first.level.tick();
    }
    run(&mut first, "save-off");
    first.save_everything(true, true, false);
    let file = dir.join("dimensions/minecraft/overworld/data/minecraft/world_border.dat");
    assert!(!file.exists());
    first.save_everything(true, true, true);
    assert!(file.exists());
    let size = first.level.world_border.get_size();
    let second = Server::new(config.clone()).unwrap();
    let border = &second.level.world_border;
    assert_eq!((border.get_size(), border.get_lerp_time(), border.get_lerp_target(), border.get_safe_zone()), (size, 75, 1000.0, 2.0));
    let small = Server::new(crate::ServerConfig { max_world_size: 200, ..config }).unwrap();
    let border = &small.level.world_border;
    assert_eq!((border.get_min_x(), border.get_max_x(), border.get_absolute_max_size()), (-200.0, 200.0, 200));
    std::fs::remove_dir_all(dir).unwrap();
}
