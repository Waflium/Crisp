use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::{CHEST_TYPE, ChestType, LIT};
use crisp_data::{blocks, items};
use crisp_protocol::nbt::Nbt;

use super::{BlockEntity, BlockEntityKind};
use crate::network::hashed_stack::HashedStack;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::container::ContainerRef;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::inventory::abstract_container_menu::{PICKUP, QUICK_MOVE};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::level::block::{BlockHitResult, click};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_LIMIT};

fn level_with_player() -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
    (level, player)
}

fn container(level: &ServerLevel, pos: BlockPos) -> ContainerRef {
    ContainerRef::block(level, pos)
}

fn dropped(level: &ServerLevel) -> Vec<ItemStack> {
    level.entities.values().filter_map(|e| match e {
        LevelEntity::Item(item) if !item.entity.is_removed() => Some(item.item.clone()),
        _ => None,
    }).collect()
}

fn place(level: &mut ServerLevel, player: EntityId, pos: BlockPos, stack: ItemStack) {
    level.player_mut(player).inventory.slots[36] = Some(stack);
    let below = pos.below();
    let hit = BlockHitResult { location: Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5), direction: Direction::Up, block_pos: below, inside: false, world_border_hit: false };
    level.use_item_on(player, InteractionHand::MainHand, hit);
}

#[test]
fn chests_get_block_entities_that_go_with_them() {
    let (mut level, _) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::CHEST.default_state(), UPDATE_ALL);
    assert!(matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Chest(_))));
    let c = container(&level, pos);
    level.set_item(c, 3, ItemStack::new(items::DIAMOND, 5));
    assert!(level.chunks.loaded_chunk(pos.chunk()).unwrap().unsaved);
    level.destroy_block(pos, true, None, UPDATE_LIMIT);
    assert!(level.get_block_entity(pos).is_none());
    let drops = dropped(&level);
    assert_eq!(drops.iter().filter(|s| s.item == items::DIAMOND).map(|s| s.count).sum::<i32>(), 5);
    assert_eq!(drops.iter().filter(|s| s.item == items::CHEST).count(), 1);
}

#[test]
fn block_entities_are_saved_with_their_chunk() {
    let (mut level, _) = level_with_player();
    let pos = BlockPos::new(2, -60, 1);
    level.set_block(pos, blocks::FURNACE.default_state(), UPDATE_ALL);
    let c = container(&level, pos);
    level.set_item(c, 0, ItemStack::new(items::RAW_IRON, 3));
    let dimension = &crisp_data::DIMENSION_TYPES[0];
    let chunk = level.chunks.chunk(pos.chunk());
    let block_entities = chunk.block_entities_for_saving(&mut Default::default());
    let saved = crate::world::level::chunk::serialization::write(chunk, crate::server::level::chunk_level::ChunkStatus::Full, block_entities, 0, &[], &[], &[]);
    let list = saved.get("block_entities").unwrap().as_list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].get("id"), Some(&Nbt::String("minecraft:furnace".into())));
    assert_eq!(list[0].get("keepPacked"), Some(&Nbt::Byte(0)));
    let mut read = crate::world::level::chunk::serialization::read(&saved, dimension, 0).unwrap().chunk;
    let entity = read.block_entities.map.get(&pos).unwrap();
    assert_eq!(entity.container_base().unwrap().items[0], ItemStack::new(items::RAW_IRON, 3));
    let mut packed = list[0].clone();
    if let Nbt::Compound(fields) = &mut packed {
        fields.retain(|(k, _)| k != "keepPacked");
        fields.push(("keepPacked".into(), Nbt::Byte(1)));
    }
    read.block_entities.map.remove(&pos);
    read.load_block_entities(&[packed]);
    assert!(read.block_entities.map.get(&pos).is_none() && read.block_entities.pending.contains_key(&pos));
}

#[test]
fn chests_side_by_side_open_as_one() {
    let (mut level, player) = level_with_player();
    let (a, b) = (BlockPos::new(0, -60, 0), BlockPos::new(1, -60, 0));
    place(&mut level, player, a, ItemStack::new(items::CHEST, 1));
    place(&mut level, player, b, ItemStack::new(items::CHEST, 1));
    assert_eq!(level.block_state(a).get(CHEST_TYPE), ChestType::Right);
    assert_eq!(level.block_state(b).get(CHEST_TYPE), ChestType::Left);
    let c = container(&level, b);
    level.set_item(c, 0, ItemStack::new(items::DIRT, 7));
    level.player_mut(player).inventory.slots[36] = None;
    click(&mut level, player, a, Direction::Up, Vec3::new(0.5, -59.0, 0.5));
    let menu = level.player(player).container_menu.as_ref().expect("an open menu");
    assert_eq!(menu.slots.len(), 54 + 36);
    let ContainerRef::Compound { first, second } = menu.container else { panic!("a double chest") };
    assert_eq!((first.0, second.0), (a, b));
    let shown = level.slot_get(player, &level.player(player).container_menu.as_ref().unwrap().slots[27].clone());
    assert_eq!(shown, ItemStack::new(items::DIRT, 7));
    for pos in [a, b] {
        assert!(matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Chest(chest)) if chest.openers_counter.open_count == 1));
    }
    level.close_container(player, false);
    assert!(matches!(level.get_block_entity(a).map(|e| &e.kind), Some(BlockEntityKind::Chest(chest)) if chest.openers_counter.open_count == 0));
}

#[test]
fn shift_clicks_move_stacks_between_chest_and_inventory() {
    let (mut level, player) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::CHEST.default_state(), UPDATE_ALL);
    level.player_mut(player).inventory.slots[36] = None;
    click(&mut level, player, pos, Direction::Up, Vec3::new(0.5, -59.0, 0.5));
    level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::DIRT, 10));
    let state_id = level.player(player).container_menu.as_ref().unwrap().state_id();
    level.handle_container_menu_click(player, state_id, 54, 0, QUICK_MOVE, Vec::new(), HashedStack::Empty).unwrap();
    let c = container(&level, pos);
    assert_eq!(level.get_item(c, 0), ItemStack::new(items::DIRT, 10));
    assert_eq!(level.player(player).inventory.slots[36], None);
    let state_id = level.player(player).container_menu.as_ref().unwrap().state_id();
    level.handle_container_menu_click(player, state_id, 0, 1, PICKUP, Vec::new(), HashedStack::Empty).unwrap();
    assert_eq!(level.player(player).carried, Some(ItemStack::new(items::DIRT, 5)));
    let state_id = level.player(player).container_menu.as_ref().unwrap().state_id();
    level.handle_container_menu_click(player, state_id, 1, 0, PICKUP, Vec::new(), HashedStack::Empty).unwrap();
    assert_eq!((level.get_item(c, 0).count, level.get_item(c, 1).count), (5, 5));
    level.player_mut(player).carried = Some(ItemStack::new(items::STONE, 2));
    level.close_container(player, true);
    assert_eq!(level.player(player).carried, None);
    assert!(level.player(player).inventory.slots.iter().flatten().any(|s| *s == ItemStack::new(items::STONE, 2)));
}

#[test]
fn trapped_chests_power_as_many_as_look_in() {
    let (mut level, player) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::TRAPPED_CHEST.default_state(), UPDATE_ALL);
    level.player_mut(player).inventory.slots[36] = None;
    let state = level.block_state(pos);
    assert_eq!(crate::world::level::block::of(state).own_signal(state, &mut level, pos), 0);
    click(&mut level, player, pos, Direction::Up, Vec3::new(0.5, -59.0, 0.5));
    assert_eq!(crate::world::level::block::of(state).own_signal(state, &mut level, pos), 1);
    assert_eq!(crate::world::level::block::of(state).get_direct_signal(state, &mut level, pos, Direction::Up), 1);
}

#[test]
fn hoppers_pull_an_item_every_eight_ticks() {
    let (mut level, _) = level_with_player();
    let hopper = BlockPos::new(0, -60, 0);
    let chest = hopper.above();
    level.set_block(hopper, blocks::HOPPER.default_state(), UPDATE_ALL);
    level.set_block(chest, blocks::CHEST.default_state(), UPDATE_ALL);
    let c = container(&level, chest);
    level.set_item(c, 0, ItemStack::new(items::DIRT, 3));
    let h = container(&level, hopper);
    let mut pulled_at = Vec::new();
    for tick in 1..=20 {
        level.tick();
        let count: i32 = (0..5).map(|i| level.get_item(h, i).count).sum();
        if count > pulled_at.len() as i32 {
            pulled_at.push(tick);
        }
    }
    assert_eq!(pulled_at, [1, 9, 17]);
    assert_eq!(level.get_item(c, 0).count, 0);
}

#[test]
fn hoppers_feed_a_furnace_from_above() {
    let (mut level, _) = level_with_player();
    let furnace = BlockPos::new(0, -60, 0);
    let hopper = furnace.above();
    level.set_block(furnace, blocks::FURNACE.default_state(), UPDATE_ALL);
    level.set_block(hopper, blocks::HOPPER.default_state(), UPDATE_ALL);
    let h = container(&level, hopper);
    level.set_item(h, 0, ItemStack::new(items::RAW_IRON, 2));
    level.tick();
    let f = container(&level, furnace);
    assert_eq!(level.get_item(f, 0), ItemStack::new(items::RAW_IRON, 1));
}

#[test]
fn furnaces_smelt_with_fuel_and_light_up() {
    let (mut level, _) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::FURNACE.default_state(), UPDATE_ALL);
    let c = container(&level, pos);
    level.set_item(c, 0, ItemStack::new(items::RAW_IRON, 2));
    level.set_item(c, 1, ItemStack::new(items::COAL, 1));
    level.tick();
    assert!(level.block_state(pos).get(LIT));
    assert_eq!(level.get_item(c, 1), ItemStack::empty().copy());
    for _ in 1..199 {
        level.tick();
    }
    assert!(level.get_item(c, 2).is_empty());
    level.tick();
    assert_eq!(level.get_item(c, 2), ItemStack::new(items::IRON_INGOT, 1));
    let Some(BlockEntityKind::Furnace(furnace)) = level.get_block_entity(pos).map(|e| &e.kind) else { panic!() };
    assert_eq!((furnace.lit_time_remaining, furnace.lit_total_time), (1600 - 199, 1600));
    assert_eq!(furnace.recipes_used, [("minecraft:iron_ingot_from_smelting_raw_iron".to_string(), 1)]);
}

#[test]
fn blast_furnaces_cook_twice_as_fast_on_half_the_fuel() {
    let (mut level, _) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::BLAST_FURNACE.default_state(), UPDATE_ALL);
    let c = container(&level, pos);
    level.set_item(c, 0, ItemStack::new(items::RAW_IRON, 1));
    level.set_item(c, 1, ItemStack::new(items::COAL, 1));
    for _ in 0..99 {
        level.tick();
    }
    assert!(level.get_item(c, 2).is_empty());
    level.tick();
    assert_eq!(level.get_item(c, 2), ItemStack::new(items::IRON_INGOT, 1));
    let Some(BlockEntityKind::Furnace(furnace)) = level.get_block_entity(pos).map(|e| &e.kind) else { panic!() };
    assert_eq!(furnace.lit_total_time, 800);
}

#[test]
fn shulker_boxes_keep_their_contents_when_broken() {
    let (mut level, player) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::SHULKER_BOX.default_state(), UPDATE_ALL);
    let c = container(&level, pos);
    level.set_item(c, 4, ItemStack::new(items::DIAMOND, 2));
    level.player_mut(player).inventory.slots[36] = None;
    level.destroy_block_by_player(player, pos);
    let drops = dropped(&level);
    assert_eq!(drops.len(), 1);
    let shulker = drops[0].clone();
    assert_eq!(shulker.item, items::SHULKER_BOX);
    assert_eq!(shulker.get(dc::CONTAINER), Some(&crate::nbt::parse_fully("[{slot:4,item:{id:\"minecraft:diamond\",count:2}}]").unwrap()));
    let again = BlockPos::new(1, -60, 0);
    place(&mut level, player, again, shulker);
    let c = container(&level, again);
    assert_eq!(level.get_item(c, 4), ItemStack::new(items::DIAMOND, 2));
    assert!(level.get_block_entity(again).unwrap().components().is_empty());
}

#[test]
fn named_chests_keep_their_name() {
    let (mut level, player) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    let mut chest = ItemStack::new(items::CHEST, 1);
    chest.set(dc::CUSTOM_NAME, Some(Nbt::String("Loot".into())));
    place(&mut level, player, pos, chest);
    assert_eq!(level.get_block_entity(pos).unwrap().container_base().unwrap().name, Some(Nbt::String("Loot".into())));
    level.player_mut(player).inventory.slots[36] = None;
    level.destroy_block_by_player(player, pos);
    let drops = dropped(&level);
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].get(dc::CUSTOM_NAME), Some(&Nbt::String("Loot".into())));
}

#[test]
fn droppers_put_items_into_the_container_they_face() {
    let (mut level, _) = level_with_player();
    let dropper = BlockPos::new(0, -60, 0);
    let chest = BlockPos::new(0, -60, -1);
    let state = blocks::DROPPER.default_state().set(crisp_data::properties::FACING, Direction::North);
    level.set_block(dropper, state, UPDATE_ALL);
    level.set_block(chest, blocks::CHEST.default_state(), UPDATE_ALL);
    let d = container(&level, dropper);
    level.set_item(d, 0, ItemStack::new(items::DIRT, 3));
    level.set_block(dropper.east(), blocks::REDSTONE_BLOCK.default_state(), UPDATE_ALL);
    for _ in 0..4 {
        level.tick();
    }
    let c = container(&level, chest);
    assert_eq!(level.get_item(c, 0), ItemStack::new(items::DIRT, 1));
    assert_eq!(level.get_item(d, 0), ItemStack::new(items::DIRT, 2));
}

fn command_server() -> crate::server::Server {
    let mut server = crate::server::Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
    server.level.chunks.chunk(crisp_core::ChunkPos::new(0, 0));
    server
}

fn run(server: &mut crate::server::Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = crate::commands::source::CommandSourceStack::server(server);
    crate::commands::perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

#[test]
fn data_commands_read_and_write_block_entities() {
    let mut server = command_server();
    assert_eq!(run(&mut server, "data get block 0 -60 0"), ["The target block is not a block entity"]);
    assert_eq!(run(&mut server, "setblock 0 -60 0 chest{Items:[{Slot:3b,id:\"minecraft:dirt\",count:2}]}"), ["Changed the block at 0, -60, 0"]);
    assert_eq!(run(&mut server, "data get block 0 -60 0 Items[0].count"), ["0, -60, 0 has the following block data: 2"]);
    assert_eq!(run(&mut server, "data merge block 0 -60 0 {CustomName:\"Box\"}"), ["Modified block data of 0, -60, 0"]);
    assert_eq!(run(&mut server, "data merge block 0 -60 0 {CustomName:\"Box\"}"), ["Nothing changed. The specified properties already have these values"]);
    assert_eq!(run(&mut server, "data get block 0 -60 0 Items[0].count 1.5"), ["Items[0].count on block 0, -60, 0 after scale factor of 1.50 is 3"]);
    assert_eq!(run(&mut server, "data modify block 0 -60 0 Items[0].count set value 5b"), ["Modified block data of 0, -60, 0"]);
    let pos = BlockPos::new(0, -60, 0);
    let c = ContainerRef::block(&server.level, pos);
    assert_eq!(server.level.get_item(c, 3), ItemStack::new(items::DIRT, 5));
    assert_eq!(run(&mut server, "data get block 0 -60 0 x"), ["0, -60, 0 has the following block data: 0"]);
    assert_eq!(run(&mut server, "execute if data block 0 -60 0 Items[{id:\"minecraft:dirt\"}]"), ["Test passed. Count: 1"]);
    assert_eq!(run(&mut server, "execute if block 0 -60 0 chest{CustomName:\"Box\"}"), ["Test passed"]);
    run(&mut server, "execute store result block 0 -60 0 Items[0].count byte 1 run data get block 0 -60 0 Items[0].count 2");
    assert_eq!(server.level.get_item(c, 3), ItemStack::new(items::DIRT, 10));
}

#[test]
fn weathered_copper_chests_keep_what_they_hold() {
    let (mut level, _) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::COPPER_CHEST.default_state(), UPDATE_ALL);
    let c = container(&level, pos);
    level.set_item(c, 0, ItemStack::new(items::DIRT, 4));
    let exposed = crate::world::level::block::weathering_copper::with_properties_of(blocks::EXPOSED_COPPER_CHEST, level.block_state(pos));
    level.set_block(pos, exposed, UPDATE_ALL);
    assert_eq!(level.get_block_entity(pos).unwrap().block_state(), exposed);
    let c = container(&level, pos);
    assert_eq!(level.get_item(c, 0), ItemStack::new(items::DIRT, 4));
    assert!(dropped(&level).is_empty());
}

#[test]
fn chunk_packets_list_block_entities() {
    let (mut level, _) = level_with_player();
    let pos = BlockPos::new(3, -60, 5);
    level.set_block(pos, blocks::CHEST.default_state(), UPDATE_ALL);
    let mut w = crisp_protocol::Writer::new();
    level.chunks.chunk(pos.chunk()).write_data(&mut w);
    let bytes = w.as_slice();
    let tail = &bytes[bytes.len() - 6..];
    assert_eq!(tail, &[1, 3 << 4 | 5, 0xff, 0xc4, crisp_data::block_entity_types::CHEST.0 as u8, 0]);
}

#[test]
fn unported_block_entities_keep_their_data() {
    let (mut level, _) = level_with_player();
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::CREAKING_HEART.default_state(), UPDATE_ALL);
    let tag = crate::nbt::parse_fully("{id:\"minecraft:creaking_heart\",x:0,y:-60,z:0,creaking:[I;1,2,3,4],extra:1b}").unwrap();
    level.get_block_entity(pos).unwrap().load_with_components(&tag);
    let saved = level.block_entity_data(pos).unwrap();
    assert_eq!(saved.get("creaking"), tag.get("creaking"));
    assert_eq!(saved.get("extra"), Some(&Nbt::Byte(1)));
    assert!(level.get_block_entity(pos).unwrap().has_update_packet());
}

#[test]
fn block_entity_types_match_their_blocks() {
    let pos = BlockPos::new(0, 0, 0);
    assert!(BlockEntity::new_for(pos, blocks::COPPER_CHEST.default_state()).is_some_and(|e| e.ty() == crisp_data::block_entity_types::CHEST));
    assert!(BlockEntity::new_for(pos, blocks::STONE.default_state()).is_none());
    assert!(BlockEntity::new(crisp_data::block_entity_types::CHEST, pos, blocks::FURNACE.default_state()).is_none());
}
