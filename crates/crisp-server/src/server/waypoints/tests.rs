use crisp_core::{BlockPos, ChunkPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_protocol::Reader;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::ServerConfig;
use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::network::protocol::game::clientbound_tracked_waypoint_packet::ClientboundTrackedWaypointPacket as Packet;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::entity::EntityId;
use crate::world::waypoints::waypoint::Icon;

fn waypoint_packets(packets: &mut UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        if Reader::new(&packet).varint().unwrap() == clientbound::WAYPOINT {
            out.push(packet);
        }
    }
    out
}

fn join(server: &mut Server, uuid: u128, name: &str, pos: Vec3) -> (EntityId, UnboundedReceiver<Vec<u8>>) {
    let id = testing::server_player_with_profile(server, Profile { uuid, name: name.into(), properties: Vec::new() }, pos, 0.0, 0.0);
    let (out, packets) = Outbound::capture();
    server.levels[0].player_mut(id).connection = out;
    (id, packets)
}

fn tick(server: &mut Server, players: &[EntityId]) {
    server.levels[0].tick();
    for &id in players {
        server.levels[0].tick_connection(id);
    }
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

#[test]
fn players_track_each_other_like_vanilla() {
    let mut server = Server::new(ServerConfig::default()).unwrap();
    let (alpha, mut alpha_packets) = join(&mut server, 1, "Alpha", Vec3::new(0.5, -60.0, 0.5));
    let (bravo, mut bravo_packets) = join(&mut server, 2, "Bravo", Vec3::new(3.5, -60.0, 0.5));
    assert!(waypoint_packets(&mut alpha_packets).is_empty());
    tick(&mut server, &[alpha, bravo]);
    let default = Icon::new();
    assert_eq!(waypoint_packets(&mut alpha_packets), [Packet::add_waypoint_position(2, &default, BlockPos::new(3, -60, 0)).encode()]);
    assert_eq!(waypoint_packets(&mut bravo_packets), [Packet::add_waypoint_position(1, &default, BlockPos::new(0, -60, 0)).encode()]);

    run(&mut server, "tp Alpha 0.5 -60 200.5");
    assert!(waypoint_packets(&mut alpha_packets).is_empty());
    assert_eq!(waypoint_packets(&mut bravo_packets), [Packet::add_waypoint_chunk(1, &default, ChunkPos::new(0, 12)).encode()]);

    assert_eq!(run(&mut server, "waypoint modify Alpha color hex F0A"), ["Waypoint color is now FF00AA"]);
    let colored = Icon { color: Some(0xFFFF00AAu32 as i32), ..Icon::new() };
    assert_eq!(waypoint_packets(&mut bravo_packets), [Packet::remove_waypoint(1).encode(), Packet::add_waypoint_chunk(1, &colored, ChunkPos::new(0, 12)).encode()]);
    assert_eq!(run(&mut server, "waypoint modify Alpha color hex 1234"), ["Invalid hex color code '1234'", "...r hex 1234<--[HERE]"]);
    let names = if bravo & 15 < alpha & 15 { "Bravo, Alpha" } else { "Alpha, Bravo" };
    assert_eq!(run(&mut server, "waypoint list"), [format!("2 waypoint(s) in minecraft:overworld: {names}")]);

    run(&mut server, "gamerule locator_bar false");
    assert_eq!(waypoint_packets(&mut alpha_packets), [Packet::remove_waypoint(2).encode()]);
    assert_eq!(waypoint_packets(&mut bravo_packets), [Packet::remove_waypoint(1).encode()]);
}
