use std::path::{Path, PathBuf};

use crisp_core::{ChunkPos, Vec3};
use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use crate::server::OVERWORLD;
use crate::server::level::chunk_cache::ChunkCache;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::primed_tnt::PrimedTnt;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity, RemovalReason};
use crate::world::item::ItemStack;
use crate::world::level::chunk::FlatLevelSource;
use crate::world::level::chunk::storage::entity_storage::chunk_tag;
use crate::world::level::chunk::storage::{RegionStorage, encode_nbt};

const ORACLE_STORED: &str = "0a00000b0008506f736974696f6e00000002000000010000000003000b4461746156657273696f6e0000139f090008456e7469746965730a000000030900064d6f74696f6e06000000033fb999999999999a0000000000000000bfa999999999999a0200064865616c7468000501000c496e76756c6e657261626c650006000d66616c6c5f64697374616e63650000000000000000020003416972012c0100084f6e47726f756e640103000e506f7274616c436f6f6c646f776e00000000090008526f746174696f6e050000000242b40000000000000a00044974656d030005636f756e7400000003080002696400116d696e6563726166743a6469616d6f6e6400090003506f7306000000034031800000000000c04e000000000000400a00000000000002000b5069636b757044656c6179000a0200044669726500000800026964000e6d696e6563726166743a6974656d0b00045555494400000004111111112222433384445555666677770200034167650000000900064d6f74696f6e06000000030000000000000000000000000000000000000000000000000200064865616c7468000501000c496e76756c6e657261626c650006000d66616c6c5f64697374616e63650000000000000000020003416972012c0100084f6e47726f756e640003000e506f7274616c436f6f6c646f776e00000000090008526f746174696f6e050000000241480000000000000a00044974656d030005636f756e74000000400800026964000f6d696e6563726166743a73746f6e6500090003506f7306000000034034000000000000c04d800000000000401800000000000002000b5069636b757044656c617900000200044669726500000800026964000e6d696e6563726166743a6974656d0b000455554944000000040123456789ab4cde8f0123456789abcd0200034167650000000900064d6f74696f6e060000000300000000000000003fc999999999999a000000000000000001000c496e76756c6e657261626c650006000d66616c6c5f64697374616e63650000000000000000020003416972012c0100084f6e47726f756e640003000e506f7274616c436f6f6c646f776e00000000090008526f746174696f6e05000000020000000000000000020004667573650021090003506f7306000000034032800000000000c04e000000000000402500000000000008000b626c6f636b5f7374617465000d6d696e6563726166743a746e740200044669726500000800026964000d6d696e6563726166743a746e740b00045555494400000004aaaaaaaaaaaaaaaacccccccccccccccc0000";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn entity_chunks_store_like_vanilla() {
    let mut diamonds = ItemEntity::with_movement(Vec3::new(17.5, -60.0, 3.25), ItemStack::new(items::DIAMOND, 3), Vec3::new(0.1, 0.0, -0.05));
    diamonds.entity.uuid = 0x1111111122224333_8444555566667777;
    diamonds.entity.y_rot = 90.0;
    diamonds.entity.on_ground = true;
    diamonds.pickup_delay = 10;
    let mut stone = ItemEntity::with_movement(Vec3::new(20.0, -59.0, 6.0), ItemStack::new(items::STONE, 64), Vec3::ZERO);
    stone.entity.uuid = 0x0123456789ab4cde_8f0123456789abcd;
    stone.entity.y_rot = 12.5;
    let mut tnt = PrimedTnt::of_type();
    tnt.entity.uuid = 0xaaaaaaaaaaaaaaaa_cccccccccccccccc;
    tnt.entity.set_pos(Vec3::new(18.5, -60.0, 10.5));
    tnt.entity.delta_movement = Vec3::new(0.0, 0.2, 0.0);
    tnt.set_fuse(33);
    let entities = [LevelEntity::Item(diamonds), LevelEntity::Item(stone), LevelEntity::PrimedTnt(tnt)];
    let tag = chunk_tag(ChunkPos::new(1, 0), entities.iter());
    assert_eq!(hex(&encode_nbt(&tag)), ORACLE_STORED, "{}", crate::nbt::to_string(&tag));
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("crisp-entities-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn level_in(dir: &Path) -> ServerLevel {
    let dimension = &crisp_data::DIMENSION_TYPES[0];
    let chunks = ChunkCache::new(dimension, FlatLevelSource::default(), Some(RegionStorage::new(dir.join("region"), false)));
    ServerLevel::new(OVERWORLD, 0, chunks, Some(RegionStorage::new(dir.join("entities"), false)), 10, 10)
}

fn add_item(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let item = ItemEntity::with_movement(pos, ItemStack::new(items::STONE, 1), Vec3::ZERO);
    let id = item.entity.id;
    assert!(level.add_fresh_entity(LevelEntity::Item(item)));
    id
}

fn stored(dir: &Path, pos: ChunkPos) -> Option<Nbt> {
    RegionStorage::new(dir.join("entities"), false).read(pos).unwrap()
}

fn uuids(entities: &Nbt) -> Vec<u128> {
    entities.get("Entities").unwrap().as_list().iter().map(|e| crate::world::entity::entity_save::uuid_from_nbt(e.get("UUID").unwrap()).unwrap()).collect()
}

fn entity_with_uuid(level: &ServerLevel, uuid: u128) -> Option<EntityId> {
    level.entities.iter().find(|(_, e)| e.entity().uuid == uuid).map(|(&id, _)| id)
}

#[test]
fn entities_survive_saving_and_loading_the_world() {
    let dir = temp_dir("reload");
    let mut level = level_in(&dir);
    testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
    let low = add_item(&mut level, Vec3::new(20.5, -60.0, 4.5));
    let high = add_item(&mut level, Vec3::new(21.5, 5.0, 5.5));
    let mut tnt = PrimedTnt::of_type();
    tnt.entity.set_pos(Vec3::new(-27.5, -60.0, 4.5));
    tnt.set_fuse(60);
    let tnt = {
        let id = tnt.entity.id;
        assert!(level.add_fresh_entity(LevelEntity::PrimedTnt(tnt)));
        id
    };
    let saved: Vec<Nbt> = [low, high, tnt].iter().map(|id| level.entities[id].save().unwrap()).collect();
    level.save_all_entities();

    let chunk = stored(&dir, ChunkPos::new(1, 0)).expect("chunk (1, 0) stored");
    let uuid = |id: EntityId| level.entities[&id].entity().uuid;
    assert_eq!(uuids(&chunk), [uuid(high), uuid(low)]);
    assert_eq!(chunk.get("Position"), Some(&Nbt::IntArray(vec![1, 0])));
    assert_eq!(chunk.get("DataVersion"), Some(&Nbt::Int(crisp_data::WORLD_VERSION)));
    assert_eq!(uuids(&stored(&dir, ChunkPos::new(-2, 0)).unwrap()), [uuid(tnt)]);

    let mut reloaded = level_in(&dir);
    testing::player(&mut reloaded, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
    for tag in saved {
        let uuid = crate::world::entity::entity_save::uuid_from_nbt(tag.get("UUID").unwrap()).unwrap();
        let id = entity_with_uuid(&reloaded, uuid).expect("entity loaded again");
        assert_eq!(reloaded.entities[&id].save(), Some(tag));
        assert!(reloaded.all_entities().any(|e| e == id), "visible");
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn move_player(level: &mut ServerLevel, player: EntityId, pos: Vec3) {
    level.player_mut(player).entity.set_pos(pos);
    level.move_player(player);
    level.finish_chunk_work();
    level.tick();
    level.finish_chunk_work();
    level.tick();
}

#[test]
fn entities_leave_with_their_chunk_and_come_back_with_it() {
    let dir = temp_dir("unload");
    let mut level = level_in(&dir);
    let player = testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
    let item = add_item(&mut level, Vec3::new(20.5, -60.0, 4.5));
    let uuid = level.entities[&item].entity().uuid;
    level.tick();

    move_player(&mut level, player, Vec3::new(2000.5, -60.0, 8.5));
    assert!(!level.entities.contains_key(&item));
    assert!(!level.all_entities().any(|e| e == item));
    assert!(!level.entity_manager.is_loaded(uuid));
    level.save_all_entities();
    assert_eq!(uuids(&stored(&dir, ChunkPos::new(1, 0)).expect("saved on unload")), [uuid]);

    move_player(&mut level, player, Vec3::new(8.5, -60.0, 8.5));
    let back = entity_with_uuid(&level, uuid).expect("item loaded with its chunk");
    assert_ne!(back, item);
    assert!(level.all_entities().any(|e| e == back));
    assert!(level.entity_manager.is_loaded(uuid));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn chunks_without_entities_store_nothing() {
    let dir = temp_dir("empty");
    let mut level = level_in(&dir);
    testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
    level.tick();
    level.save_all_entities();
    let region_files = || std::fs::read_dir(dir.join("entities")).map_or(0, |files| files.count());
    assert_eq!(region_files(), 0, "no entity ever, no region file");

    let item = add_item(&mut level, Vec3::new(20.5, -60.0, 4.5));
    level.save_all_entities();
    assert!(stored(&dir, ChunkPos::new(1, 0)).is_some());
    level.entity_mut(item).removal = Some(RemovalReason::Discarded);
    level.remove_entity(item);
    level.save_all_entities();
    assert_eq!(stored(&dir, ChunkPos::new(1, 0)), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn entities_tick_only_in_entity_ticking_chunks() {
    let mut level = testing::level();
    level.set_simulation_distance(2);
    let player = testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
    let ticking = |level: &ServerLevel, id: EntityId| level.tick_list.iter().any(|e| e == id);
    let near = add_item(&mut level, Vec3::new(8.5, -60.0, 20.5));
    let outside = add_item(&mut level, Vec3::new(6.0 * 16.0 + 8.5, -60.0, 8.5));
    let far = add_item(&mut level, Vec3::new(11.0 * 16.0 + 8.5, -60.0, 8.5));
    assert!(level.entity_manager.is_ticking(ChunkPos::new(6, 0)));
    assert!(!level.entity_manager.is_ticking(ChunkPos::new(11, 0)));
    for _ in 0..3 {
        level.tick();
    }
    assert_eq!(level.entity(near).tick_count, 3);
    assert_eq!(level.entity(outside).tick_count, 0);
    assert!(ticking(&level, outside));
    assert_eq!(level.entity(far).tick_count, 0);
    assert!(!ticking(&level, far));
    assert!(level.all_entities().any(|e| e == far));
    assert!(level.chunk_map.tracked.contains_key(far));

    level.entity_mut(near).set_pos(Vec3::new(11.0 * 16.0 + 4.5, -60.0, 8.5));
    level.entity_moved(near);
    assert!(!ticking(&level, near));
    level.tick();
    assert_eq!(level.entity(near).tick_count, 3);

    move_player(&mut level, player, Vec3::new(11.0 * 16.0 + 8.5, -60.0, 8.5));
    assert!(level.entity_manager.is_ticking(ChunkPos::new(11, 0)));
    assert!(ticking(&level, near) && ticking(&level, far));
    let (near_ticks, far_ticks) = (level.entity(near).tick_count, level.entity(far).tick_count);
    assert!(far_ticks > 0);
    level.tick();
    assert_eq!((level.entity(near).tick_count, level.entity(far).tick_count), (near_ticks + 1, far_ticks + 1));
}
