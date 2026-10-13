use crisp_core::{ChunkPos, Vec3};
use crisp_protocol::nbt::Nbt;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::world::entity::EntityId;

fn server() -> Server {
    let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
    for x in -2..=2 {
        for z in -2..=2 {
            server.levels[0].chunks.chunk(ChunkPos::new(x, z));
        }
    }
    server
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

fn entity_with_uuid(server: &Server, last: i32) -> EntityId {
    let uuid = (1u128 << 96) | (2u128 << 64) | (3u128 << 32) | last as u32 as u128;
    server.levels[0].entity_manager.visible_entity_storage.get(uuid).expect("entity with that uuid")
}

fn step(server: &mut Server, vehicle: EntityId) {
    server.levels[0].tick_non_passenger(vehicle);
}

#[test]
fn minecart_rolls_along_a_straight_rail_like_vanilla() {
    let mut server = server();
    run(&mut server, "fill 0 -60 0 30 -60 0 minecraft:rail[shape=east_west]");
    run(&mut server, "summon minecart 0.5 -59.9375 0.5 {Motion:[0.4d,0.0d,0.0d],UUID:[I;1,2,3,4]}");
    let cart = entity_with_uuid(&server, 4);
    let expected = [0.9, 1.284, 1.6526399999999999, 2.0065343999999996, 2.3462730239999994, 2.6724221030399993, 2.985525218918399, 3.2861042101616627];
    for x in expected {
        step(&mut server, cart);
        assert_eq!(server.levels[0].entity(cart).position(), Vec3::new(x, -59.9375, 0.5));
    }
}

#[test]
fn boats_fall_and_float_like_vanilla() {
    let mut server = server();
    run(&mut server, "fill 0 -61 10 12 -62 22 minecraft:water");
    run(&mut server, "summon oak_boat 2.5 -58 12.5 {UUID:[I;1,2,3,1]}");
    run(
        &mut server,
        "summon oak_boat 4.5 -60.5 15.5 {Motion:[0.3d,0.0d,0.1d],Rotation:[30.0f,0.0f],UUID:[I;1,2,3,2],Passengers:[{id:\"minecraft:chicken\",NoAI:1b,UUID:[I;1,2,3,3]}]}",
    );
    let (falling, carrying, chicken) = (entity_with_uuid(&server, 1), entity_with_uuid(&server, 2), entity_with_uuid(&server, 3));
    assert_eq!(server.levels[0].vehicle(chicken), Some(carrying));
    let falling_y = [-58.04, -58.12, -58.239999999999995];
    let carrying_positions = [
        Vec3::new(4.769999992847443, -60.4980911333133, 15.589999997615815),
        Vec3::new(5.0129999799728395, -60.49490724157049, 15.67099999332428),
        Vec3::new(5.231699962592126, -60.491028323434996, 15.743899987530709),
    ];
    for (y, position) in falling_y.into_iter().zip(carrying_positions) {
        step(&mut server, falling);
        step(&mut server, carrying);
        assert_eq!(server.levels[0].entity(falling).position(), Vec3::new(2.5, y, 12.5));
        assert_eq!(server.levels[0].entity(carrying).position(), position);
    }
}

#[test]
fn ride_command_mounts_and_dismounts() {
    let mut server = server();
    run(&mut server, "summon minecart 0.5 -60 0.5 {UUID:[I;1,2,3,1]}");
    run(&mut server, "summon chicken 3.5 -60 3.5 {NoAI:1b,UUID:[I;1,2,3,2]}");
    let cart = "@e[limit=1,nbt={UUID:[I;1,2,3,1]}]";
    let chicken = "@e[limit=1,nbt={UUID:[I;1,2,3,2]}]";
    assert_eq!(run(&mut server, &format!("ride {chicken} mount {cart}")), ["Chicken started riding Minecart"]);
    assert_eq!(run(&mut server, &format!("ride {chicken} mount {cart}")), ["Chicken is already riding Minecart"]);
    assert_eq!(run(&mut server, &format!("ride {cart} mount {chicken}")), ["Can't mount entity on itself or any of its passengers"]);
    assert_eq!(run(&mut server, &format!("ride {cart} dismount")), ["Minecart is not riding any vehicle"]);
    assert_eq!(run(&mut server, &format!("ride {chicken} dismount")), ["Chicken stopped riding Minecart"]);
}

#[test]
fn passengers_are_saved_inside_their_vehicle() {
    let mut server = server();
    run(&mut server, "summon minecart 0.5 -60 0.5 {UUID:[I;1,2,3,1],Passengers:[{id:\"minecraft:chicken\",NoAI:1b,UUID:[I;1,2,3,2]}]}");
    let (cart, chicken) = (entity_with_uuid(&server, 1), entity_with_uuid(&server, 2));
    let level = &server.levels[0];
    assert_eq!(level.save_entity(chicken), None);
    let saved = level.save_entity(cart).expect("a saved minecart");
    let passengers = saved.get("Passengers").map(Nbt::as_list).expect("a passenger list");
    assert_eq!(passengers.len(), 1);
    assert_eq!(passengers[0].get("id").and_then(Nbt::as_str), Some("minecraft:chicken"));
    let level = &mut server.levels[0];
    let loaded = level.load_entity_recursive(&saved, &mut |_| {});
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].entity().passengers, [loaded[1].entity().id]);
    assert_eq!(loaded[1].entity().vehicle, Some(loaded[0].entity().id));
}
