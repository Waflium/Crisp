use crisp_core::{AABB, Axis, BlockPos, ChunkPos, Vec3};
use crisp_protocol::nbt::Nbt;

use super::world_border::{MAX_SIZE, Settings, WorldBorder};
use crate::network::connection::Outbound;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::{EntityId, MoverType};
use crate::world::level::collision::{Collider, shapes_collide};

fn border() -> WorldBorder {
    WorldBorder::new(Settings::DEFAULT, None)
}

fn bounds(border: &WorldBorder) -> [f64; 4] {
    [border.get_min_x(), border.get_min_z(), border.get_max_x(), border.get_max_z()]
}

fn collide(border: &WorldBorder, axis: Axis, moving: &AABB, distance: f64) -> f64 {
    shapes_collide(axis, moving, &[Collider { shape: border.get_collision_shape(), offset: [0.0; 3] }], distance)
}

fn shape_coords(border: &WorldBorder) -> [Vec<f64>; 3] {
    Axis::VALUES.map(|axis| border.get_collision_shape().coords(axis).to_vec())
}

#[test]
fn default_border() {
    let border = border();
    assert_eq!(border.get_size(), 5.9999968E7);
    assert_eq!(bounds(&border), [-2.9999984E7, -2.9999984E7, 2.9999984E7, 2.9999984E7]);
    assert_eq!((border.get_warning_time(), border.get_warning_blocks()), (300, 5));
    assert_eq!((border.get_damage_per_block(), border.get_safe_zone()), (0.2, 5.0));
    assert_eq!((border.get_lerp_time(), border.get_lerp_target()), (0, 5.9999968E7));
    let inf = f64::INFINITY;
    assert_eq!(shape_coords(&border), [vec![-inf, -2.9999984E7, 2.9999984E7, inf], vec![-inf, inf], vec![-inf, -2.9999984E7, 2.9999984E7, inf]]);
    assert_eq!(Settings::of(&border), Settings { lerp_target: MAX_SIZE, ..Settings::DEFAULT });
}

#[test]
fn static_bounds_distances_and_shape() {
    let mut border = border();
    border.set_center(10.5, -3.25);
    border.set_size(100.0);
    assert_eq!(bounds(&border), [-39.5, -53.25, 60.5, 46.75]);
    let inf = f64::INFINITY;
    assert_eq!(shape_coords(&border), [vec![-inf, -40.0, 61.0, inf], vec![-inf, inf], vec![-inf, -54.0, 47.0, inf]]);
    #[allow(clippy::type_complexity)]
    let points: [((f64, f64), f64, bool, bool, (f64, f64), (i32, i32)); 7] = [
        ((10.5, -3.25), 50.0, true, true, (10.5, -3.25), (10, -4)),
        ((60.0, 0.0), 0.5, true, true, (60.0, 0.0), (60, 0)),
        ((60.5, 0.0), 0.0, false, true, (60.49999000000025, 0.0), (60, 0)),
        ((61.0, 4.0), -0.5, false, false, (60.49999000000025, 4.0), (60, 4)),
        ((-40.0, -53.25), -0.5, false, true, (-39.5, -53.25), (-40, -54)),
        ((-50.0, 100.0), -53.25, false, false, (-39.5, 46.74999000000025), (-40, 46)),
        ((0.0, 46.75), 0.0, false, true, (0.0, 46.74999000000025), (0, 46)),
    ];
    for ((x, z), distance, within, within_margin, clamped, block) in points {
        assert_eq!(border.get_distance_to_border(x, z), distance, "{x} {z}");
        assert_eq!((border.is_within_bounds(x, z), border.is_within_bounds_margin(x, z, 0.5)), (within, within_margin), "{x} {z}");
        let v = border.clamp_vec3_to_bound(x, 64.0, z);
        assert_eq!((v.x, v.y, v.z), (clamped.0, 64.0, clamped.1), "{x} {z}");
        assert_eq!(border.clamp_to_bounds(x, 64.0, z), BlockPos::new(block.0, 64, block.1), "{x} {z}");
    }
    let boxes = [
        (AABB::new(59.0, 0.0, 0.0, 59.6, 1.8, 0.6), true, [1.3999999999999986, -99.0, -54.0, -3.0]),
        (AABB::new(59.7, 0.0, 0.0, 60.3, 1.8, 0.6), true, [0.7000000000000028, -99.7, -54.0, -3.0]),
        (AABB::new(60.6, 0.0, 0.0, 61.2, 1.8, 0.6), false, [5.0, -100.0, -54.0, -3.0]),
        (AABB::new(-39.0, 0.0, -53.0, -38.4, 1.8, -52.4), true, [5.0, -1.0, -1.0, -3.0]),
    ];
    for (aabb, within, expected) in boxes {
        assert_eq!(border.is_within_bounds_aabb(&aabb), within, "{aabb:?}");
        let got = [collide(&border, Axis::X, &aabb, 5.0), collide(&border, Axis::X, &aabb, -100.0), collide(&border, Axis::Z, &aabb, -60.0), collide(&border, Axis::Y, &aabb, -3.0)];
        assert_eq!(got, expected, "{aabb:?}");
    }
    let blocks = [BlockPos::new(60, 0, 0), BlockPos::new(59, 0, 46), BlockPos::new(-40, 0, -54)].map(|pos| border.is_within_bounds_pos(pos));
    assert_eq!(blocks, [true, true, false]);
}

const LERP: &str = "
    1 98.4 49 -39.5 -53.25 60.5 46.75
    2 96.8 48 -38.7 -52.45 59.7 45.95
    3 95.2 47 -37.9 -51.65 58.9 45.15
    4 93.6 46 -37.1 -50.85 58.1 44.35
    5 92.0 45 -36.3 -50.05 57.3 43.55
    6 90.4 44 -35.5 -49.25 56.5 42.75
    7 88.8 43 -34.7 -48.45 55.7 41.95
    8 87.2 42 -33.9 -47.65 54.9 41.15
    9 85.6 41 -33.1 -46.85 54.1 40.35
    10 84.0 40 -32.3 -46.05 53.3 39.55
    11 82.4 39 -31.5 -45.25 52.5 38.75
    12 80.8 38 -30.700000000000003 -44.45 51.7 37.95
    13 79.2 37 -29.9 -43.65 50.9 37.15
    14 77.6 36 -29.1 -42.85 50.1 36.35
    15 76.0 35 -28.299999999999997 -42.05 49.3 35.55
    16 74.4 34 -27.5 -41.25 48.5 34.75
    17 72.8 33 -26.700000000000003 -40.45 47.7 33.95
    18 71.2 32 -25.9 -39.65 46.9 33.15
    19 69.6 31 -25.1 -38.85 46.1 32.35
    20 68.0 30 -24.299999999999997 -38.05 45.3 31.549999999999997
    21 66.4 29 -23.5 -37.25 44.5 30.75
    22 64.8 28 -22.700000000000003 -36.45 43.7 29.950000000000003
    23 63.199999999999996 27 -21.9 -35.65 42.9 29.15
    24 61.6 26 -21.099999999999998 -34.849999999999994 42.099999999999994 28.349999999999998
    25 60.0 25 -20.3 -34.05 41.3 27.55
    26 58.4 24 -19.5 -33.25 40.5 26.75
    27 56.8 23 -18.7 -32.45 39.7 25.95
    28 55.199999999999996 22 -17.9 -31.65 38.9 25.15
    29 53.6 21 -17.099999999999998 -30.849999999999998 38.099999999999994 24.349999999999998
    30 52.0 20 -16.3 -30.05 37.3 23.55
    31 50.4 19 -15.5 -29.25 36.5 22.75
    32 48.8 18 -14.7 -28.45 35.7 21.95
    33 47.199999999999996 17 -13.899999999999999 -27.65 34.9 21.15
    34 45.599999999999994 16 -13.099999999999998 -26.849999999999998 34.099999999999994 20.349999999999998
    35 44.0 15 -12.299999999999997 -26.049999999999997 33.3 19.549999999999997
    36 42.400000000000006 14 -11.5 -25.25 32.5 18.75
    37 40.8 13 -10.700000000000003 -24.450000000000003 31.700000000000003 17.950000000000003
    38 39.2 12 -9.899999999999999 -23.65 30.9 17.15
    39 37.599999999999994 11 -9.100000000000001 -22.85 30.1 16.35
    40 36.0 10 -8.299999999999997 -22.049999999999997 29.299999999999997 15.549999999999997
    41 34.400000000000006 9 -7.5 -21.25 28.5 14.75
    42 32.8 8 -6.700000000000003 -20.450000000000003 27.700000000000003 13.950000000000003
    43 31.200000000000003 7 -5.899999999999999 -19.65 26.9 13.149999999999999
    44 29.599999999999994 6 -5.100000000000001 -18.85 26.1 12.350000000000001
    45 28.0 5 -4.299999999999997 -18.049999999999997 25.299999999999997 11.549999999999997
    46 26.39999999999999 4 -3.5 -17.25 24.5 10.75
    47 24.80000000000001 3 -2.6999999999999957 -16.449999999999996 23.699999999999996 9.949999999999996
    48 23.200000000000003 2 -1.9000000000000057 -15.650000000000006 22.900000000000006 9.150000000000006
    49 21.599999999999994 1 -1.1000000000000014 -14.850000000000001 22.1 8.350000000000001
    50 20.0 0 0.5 -13.25 20.5 6.75
    51 20.0 0 0.5 -13.25 20.5 6.75
    52 20.0 0 0.5 -13.25 20.5 6.75
";

#[test]
fn moving_border_follows_vanilla_tick_by_tick() {
    let mut border = border();
    border.set_center(10.5, -3.25);
    border.set_size(100.0);
    border.lerp_size_between(100.0, 20.0, 50);
    assert_eq!((border.get_size(), border.get_lerp_time(), border.get_lerp_target()), (100.0, 50, 20.0));
    for line in LERP.trim().lines() {
        let v: Vec<f64> = line.split_whitespace().map(|n| n.parse().unwrap()).collect();
        border.tick();
        assert_eq!((border.get_size(), border.get_lerp_time() as f64), (v[1], v[2]), "tick {}", v[0]);
        assert_eq!(bounds(&border), [v[3], v[4], v[5], v[6]], "tick {}", v[0]);
        if v[0] == 20.0 {
            let inf = f64::INFINITY;
            assert_eq!(shape_coords(&border), [vec![-inf, -25.0, 46.0, inf], vec![-inf, inf], vec![-inf, -39.0, 32.0, inf]]);
            assert_eq!(collide(&border, Axis::X, &AABB::new(40.0, 0.0, 0.0, 40.6, 1.8, 0.6), 10.0), 5.399999999999999);
            let settings = Settings { center_x: 10.5, center_z: -3.25, size: 68.0, lerp_time: 30, lerp_target: 20.0, ..Settings::DEFAULT };
            assert_eq!(Settings::of(&border), settings);
        }
    }
}

#[test]
fn odd_lerps() {
    let mut border = border();
    border.lerp_size_between(7.3, 1000.1, 3);
    let expected = [
        (338.23333333333335, 2, 3.65),
        (669.1666666666666, 1, 169.11666666666667),
        (1000.1, 0, 500.05),
        (1000.1, 0, 500.05),
    ];
    for (size, lerp_time, half) in expected {
        border.tick();
        assert_eq!((border.get_size(), border.get_lerp_time()), (size, lerp_time));
        assert_eq!(bounds(&border), [-half, -half, half, half]);
    }
    border.lerp_size_between(1000.1, 1000.1, 30);
    assert_eq!((border.get_size(), border.get_lerp_time()), (1000.1, 0));
    border.lerp_size_between(10.0, 30.0, 1);
    assert_eq!((border.get_size(), border.get_lerp_time()), (10.0, 1));
    border.tick();
    assert_eq!((border.get_size(), border.get_lerp_time()), (30.0, 0));
}

#[test]
fn absolute_max_size_clamps_the_bounds() {
    let mut border = border();
    border.set_center(40.0, 0.0);
    border.set_size(100.0);
    border.set_absolute_max_size(50);
    assert_eq!(bounds(&border), [-10.0, -50.0, 50.0, 50.0]);
    border.set_absolute_max_size(1);
    border.set_center(1000.0, 1000.0);
    assert_eq!(bounds(&border), [1.0; 4]);
    let inf = f64::INFINITY;
    assert_eq!(shape_coords(&border), [vec![-inf, inf], vec![-inf, inf], vec![-inf, inf]]);
    border.lerp_size_between(100.0, 50.0, 10);
    border.tick();
    assert_eq!(bounds(&border), [1.0; 4]);
}

#[test]
fn settings_codec() {
    let settings = |entries: Vec<(&str, Nbt)>| Nbt::Compound(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    let far = || {
        vec![
            ("center_x", Nbt::Double(3.0e7)),
            ("center_z", Nbt::Double(0.0)),
            ("damage_per_block", Nbt::Double(0.5)),
            ("safe_zone", Nbt::Double(2.0)),
            ("warning_blocks", Nbt::Int(3)),
            ("warning_time", Nbt::Int(10)),
            ("size", Nbt::Double(50.0)),
            ("lerp_time", Nbt::Long(0)),
            ("lerp_target", Nbt::Double(50.0)),
        ]
    };
    assert_eq!(Settings::from_nbt(&settings(far())), None);
    let mut numbers = far();
    numbers[0].1 = Nbt::Int(7);
    numbers[6].1 = Nbt::Float(12.5);
    numbers[7].1 = Nbt::Byte(4);
    numbers[5].1 = Nbt::Short(9);
    let mut border = WorldBorder::new(Settings::from_nbt(&settings(numbers)).unwrap(), None);
    assert_eq!((border.get_center_x(), border.get_size(), border.get_lerp_time(), border.get_lerp_target(), border.get_warning_time()), (7.0, 12.5, 4, 50.0, 9));
    border.tick();
    assert_eq!(border.get_lerp_time(), 3);
    let mut missing = far();
    missing.remove(3);
    assert_eq!(Settings::from_nbt(&settings(missing)), None);
    let settings = Settings { center_x: 1.5, warning_time: 20, lerp_time: 7, ..Settings::DEFAULT };
    assert_eq!(Settings::from_nbt(&settings.to_nbt()), Some(settings));
}

#[test]
fn saved_and_loaded_mid_lerp() {
    let dir = std::env::temp_dir().join(format!("crisp-world-border-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut border = WorldBorder::load(Some(dir.clone()));
    border.save().unwrap();
    let file = dir.join("minecraft/world_border.dat");
    assert!(file.exists());
    std::fs::remove_file(&file).unwrap();
    border.save().unwrap();
    assert!(!file.exists(), "nothing changed");
    border.set_center(-8.0, 3.0);
    border.lerp_size_between(200.0, 100.0, 40);
    for _ in 0..10 {
        border.tick();
    }
    border.save().unwrap();
    let saved = crate::world::level::storage::read_dat(&file).unwrap().unwrap();
    assert_eq!(saved.get("DataVersion").and_then(Nbt::as_i64), Some(crisp_data::WORLD_VERSION as i64));
    let mut loaded = WorldBorder::load(Some(dir.clone()));
    assert_eq!((loaded.get_center_x(), loaded.get_center_z(), loaded.get_size(), loaded.get_lerp_time(), loaded.get_lerp_target()), (-8.0, 3.0, 175.0, 30, 100.0));
    for _ in 0..30 {
        loaded.tick();
    }
    assert_eq!((loaded.get_size(), loaded.get_lerp_time()), (100.0, 0));
    std::fs::remove_dir_all(dir).unwrap();
}

fn level_with_player(pos: Vec3) -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    for x in -3..=3 {
        for z in -3..=3 {
            level.chunks.chunk(ChunkPos::new(x, z));
        }
    }
    let id = testing::player(&mut level, pos, 0.0, 0.0);
    level.handle_player_loaded(id);
    level.entity_mut(id).on_ground = true;
    (level, id)
}

#[test]
fn entities_collide_with_the_border() {
    let (mut level, id) = level_with_player(Vec3::new(8.0, -60.0, 0.5));
    level.world_border.set_size(20.0);
    level.move_entity(id, MoverType::SelfMove, Vec3::new(3.0, 0.0, 0.0));
    let e = level.entity(id);
    assert!(e.horizontal_collision);
    assert!((e.bounding_box().max_x - 10.0).abs() < 1.0e-9, "{}", e.x());
    assert!(!level.no_collision_for(id, &e.bounding_box().move_by(0.1, 0.0, 0.0)));
    assert!(level.no_collision(&e.bounding_box().move_by(0.1, 0.0, 0.0)));
    level.entity_mut(id).set_pos(Vec3::new(10.5, -60.0, 0.5));
    level.move_entity(id, MoverType::SelfMove, Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(level.entity(id).x(), 9.5);
    level.entity_mut(id).set_pos(Vec3::new(30.5, -60.0, 0.5));
    level.move_entity(id, MoverType::SelfMove, Vec3::new(-2.0, 0.0, 0.0));
    assert_eq!(level.entity(id).x(), 28.5);
}

#[test]
fn players_outside_the_safe_zone_are_hurt() {
    let (mut level, id) = level_with_player(Vec3::new(20.5, -60.0, 0.5));
    level.world_border.set_size(10.0);
    level.game_rules.set(crisp_data::game_rule::NATURAL_HEALTH_REGENERATION, 0);
    let mut health = Vec::new();
    for _ in 0..21 {
        level.tick();
        level.tick_connection(id);
        health.push(level.player(id).health);
    }
    let mut expected = vec![18.0; 10];
    expected.extend([16.0; 10]);
    expected.push(14.0);
    assert_eq!(health, expected);

    let (mut level, id) = level_with_player(Vec3::new(8.5, -60.0, 0.5));
    level.world_border.set_size(10.0);
    level.game_rules.set(crisp_data::game_rule::NATURAL_HEALTH_REGENERATION, 0);
    level.tick();
    level.tick_connection(id);
    assert_eq!(level.player(id).health, 20.0);
    level.entity_mut(id).set_pos(Vec3::new(12.5, -60.0, 0.5));
    level.tick();
    level.tick_connection(id);
    assert_eq!(level.player(id).health, 19.0);
    level.world_border.set_damage_per_block(0.0);
    level.entity_mut(id).set_pos(Vec3::new(40.5, -60.0, 0.5));
    for _ in 0..20 {
        level.tick();
        level.tick_connection(id);
    }
    assert_eq!(level.player(id).health, 19.0);
}

#[test]
fn changes_are_sent_to_the_players() {
    let (mut level, id) = level_with_player(Vec3::new(0.5, -60.0, 0.5));
    let (out, mut rx) = Outbound::capture();
    level.world_border.listener.add_connection(id, out);
    let border = &mut level.world_border;
    border.set_center(1.5, -2.0);
    border.set_size(64.0);
    border.lerp_size_between(64.0, 32.0, 100);
    border.set_warning_time(20);
    border.set_warning_blocks(7);
    border.set_damage_per_block(1.0);
    border.set_safe_zone(1.0);
    border.tick();
    let mut sent = Vec::new();
    while let Ok(packet) = rx.try_recv() {
        sent.push(packet);
    }
    let packet = |id: i32, body: fn(&mut crisp_protocol::Writer)| crate::network::connection::encode(id, body);
    use crisp_data::packets::play::clientbound;
    assert_eq!(
        sent,
        [
            packet(clientbound::SET_BORDER_CENTER, |p| {
                p.f64(1.5);
                p.f64(-2.0);
            }),
            packet(clientbound::SET_BORDER_SIZE, |p| p.f64(64.0)),
            packet(clientbound::SET_BORDER_LERP_SIZE, |p| {
                p.f64(64.0);
                p.f64(32.0);
                p.varlong(100);
            }),
            packet(clientbound::SET_BORDER_WARNING_DELAY, |p| p.varint(20)),
            packet(clientbound::SET_BORDER_WARNING_DISTANCE, |p| p.varint(7)),
        ]
    );
    level.world_border.listener.remove_connection(id);
    level.world_border.set_size(10.0);
    assert!(rx.try_recv().is_err());
}
