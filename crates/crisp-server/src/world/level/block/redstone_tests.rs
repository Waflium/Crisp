use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::*;
use crisp_data::{BlockState, Item, blocks, items};

use super::BlockHitResult;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

fn setup() -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(12.5, -60.0, 12.5), 180.0, 0.0);
    (level, player)
}

fn set(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    level.set_block(pos, state, UPDATE_ALL);
}

fn ticks(level: &mut ServerLevel, n: i32) {
    for _ in 0..n {
        level.tick();
    }
}

fn click(level: &mut ServerLevel, player: EntityId, item: Option<Item>, pos: BlockPos, face: Direction) {
    level.player_mut(player).inventory.slots[36] = item.map(|item| ItemStack::new(item, 1));
    let location = Vec3::new(
        pos.x as f64 + 0.5 + face.step_x() as f64 * 0.5,
        pos.y as f64 + 0.5 + face.step_y() as f64 * 0.5,
        pos.z as f64 + 0.5 + face.step_z() as f64 * 0.5,
    );
    let hit = BlockHitResult { location, direction: face, block_pos: pos, inside: false, world_border_hit: false };
    level.use_item_on(player, InteractionHand::MainHand, hit);
}

const GROUND: i32 = -60;

fn at(x: i32, z: i32) -> BlockPos {
    BlockPos::new(x, GROUND, z)
}

#[test]
fn torches_pop_off_when_their_support_goes() {
    let (mut level, _) = setup();
    let torch = at(0, 0);
    set(&mut level, torch, blocks::TORCH.default_state());
    set(&mut level, torch.below(), blocks::AIR.default_state());
    assert!(level.block_state(torch).is_air());

    let stone = at(2, 0);
    let wall_torch = at(2, 1);
    set(&mut level, stone, blocks::STONE.default_state());
    set(&mut level, wall_torch, blocks::WALL_TORCH.default_state().set(HORIZONTAL_FACING, Direction::South));
    assert!(level.block_state(wall_torch).is_block(blocks::WALL_TORCH));
    set(&mut level, stone, blocks::AIR.default_state());
    assert!(level.block_state(wall_torch).is_air());
}

#[test]
fn wall_torch_placement_faces_away_from_the_clicked_side() {
    let (mut level, player) = setup();
    let stone = at(0, 0);
    set(&mut level, stone, blocks::STONE.default_state());
    click(&mut level, player, Some(items::REDSTONE_TORCH), stone, Direction::East);
    let state = level.block_state(stone.east());
    assert!(state.is_block(blocks::REDSTONE_WALL_TORCH));
    assert_eq!(state.get(HORIZONTAL_FACING), Direction::East);
}

#[test]
fn lever_powers_the_block_it_is_on_and_a_lamp_behind_it() {
    let (mut level, player) = setup();
    let stone = at(0, 0);
    let lamp = at(-1, 0);
    set(&mut level, stone, blocks::STONE.default_state());
    set(&mut level, lamp, blocks::REDSTONE_LAMP.default_state());
    click(&mut level, player, Some(items::LEVER), stone, Direction::East);
    let lever = stone.east();
    let state = level.block_state(lever);
    assert_eq!(state.get(ATTACH_FACE), AttachFace::Wall);
    assert_eq!(state.get(HORIZONTAL_FACING), Direction::East);

    click(&mut level, player, None, lever, Direction::East);
    assert!(level.block_state(lever).get(POWERED));
    assert_eq!(level.get_direct_signal_to(stone), 15);
    assert!(level.block_state(lamp).get(LIT));

    click(&mut level, player, None, lever, Direction::East);
    assert!(level.block_state(lamp).get(LIT));
    ticks(&mut level, 4);
    assert!(!level.block_state(lamp).get(LIT));
}

#[test]
fn button_releases_after_its_delay() {
    let (mut level, player) = setup();
    let stone = at(0, 0);
    set(&mut level, stone, blocks::STONE.default_state());
    click(&mut level, player, Some(items::STONE_BUTTON), stone, Direction::Up);
    let button = stone.above();
    assert_eq!(level.block_state(button).get(ATTACH_FACE), AttachFace::Floor);
    click(&mut level, player, None, button, Direction::Up);
    assert!(level.block_state(button).get(POWERED));
    assert_eq!(level.get_direct_signal_to(stone), 15);
    ticks(&mut level, 19);
    assert!(level.block_state(button).get(POWERED));
    ticks(&mut level, 1);
    assert!(!level.block_state(button).get(POWERED));
    assert_eq!(level.get_direct_signal_to(stone), 0);
}

#[test]
fn redstone_torch_turns_off_when_its_block_is_powered() {
    let (mut level, player) = setup();
    let stone = at(0, 0);
    let torch = stone.above();
    set(&mut level, stone, blocks::STONE.default_state());
    set(&mut level, torch, blocks::REDSTONE_TORCH.default_state());
    assert_eq!(level.get_direct_signal_to(torch.above()), 15);
    click(&mut level, player, Some(items::LEVER), stone, Direction::East);
    let lever = stone.east();
    click(&mut level, player, None, lever, Direction::East);
    ticks(&mut level, 1);
    assert!(level.block_state(torch).get(LIT));
    ticks(&mut level, 1);
    assert!(!level.block_state(torch).get(LIT));
    assert_eq!(level.get_direct_signal_to(torch.above()), 0);
}

#[test]
fn redstone_torch_burns_out_after_eight_quick_toggles() {
    let (mut level, player) = setup();
    let stone = at(0, 0);
    let torch = stone.above();
    set(&mut level, stone, blocks::STONE.default_state());
    set(&mut level, torch, blocks::REDSTONE_TORCH.default_state());
    click(&mut level, player, Some(items::LEVER), stone, Direction::East);
    let lever = stone.east();
    for _ in 0..8 {
        click(&mut level, player, None, lever, Direction::East);
        ticks(&mut level, 2);
        assert!(!level.block_state(torch).get(LIT));
        click(&mut level, player, None, lever, Direction::East);
        ticks(&mut level, 2);
    }
    assert!(!level.block_state(torch).get(LIT));
    ticks(&mut level, 160);
    assert!(level.block_state(torch).get(LIT));
}

#[test]
fn redstone_wire_connects_and_loses_power_per_block() {
    let (mut level, player) = setup();
    for x in 0..4 {
        click(&mut level, player, Some(items::REDSTONE), at(x, 0).below(), Direction::Up);
    }
    let first = level.block_state(at(0, 0));
    assert_eq!(first.get(EAST_REDSTONE), RedstoneSide::Side);
    assert_eq!(first.get(WEST_REDSTONE), RedstoneSide::Side);
    assert_eq!(first.get(NORTH_REDSTONE), RedstoneSide::None);
    assert_eq!(first.get(SOUTH_REDSTONE), RedstoneSide::None);

    set(&mut level, at(-1, 0), blocks::REDSTONE_BLOCK.default_state());
    let powers: Vec<i32> = (0..4).map(|x| level.block_state(at(x, 0)).get(POWER)).collect();
    assert_eq!(powers, [15, 14, 13, 12]);

    set(&mut level, at(-1, 0), blocks::AIR.default_state());
    assert!((0..4).all(|x| level.block_state(at(x, 0)).get(POWER) == 0));
}

#[test]
fn redstone_wire_climbs_onto_a_block_and_pops_off_without_support() {
    let (mut level, player) = setup();
    let stone = at(1, 0);
    set(&mut level, stone, blocks::STONE.default_state());
    click(&mut level, player, Some(items::REDSTONE), stone, Direction::Up);
    click(&mut level, player, Some(items::REDSTONE), at(0, 0).below(), Direction::Up);
    assert_eq!(level.block_state(at(0, 0)).get(EAST_REDSTONE), RedstoneSide::Up);

    set(&mut level, at(-1, 0), blocks::REDSTONE_BLOCK.default_state());
    assert_eq!(level.block_state(stone.above()).get(POWER), 14);

    set(&mut level, stone, blocks::AIR.default_state());
    assert!(level.block_state(stone.above()).is_air());
    assert_eq!(level.block_state(at(0, 0)).get(EAST_REDSTONE), RedstoneSide::Side);
}

#[test]
fn using_a_lone_wire_toggles_between_cross_and_dot() {
    let (mut level, player) = setup();
    let wire = at(0, 0);
    click(&mut level, player, Some(items::REDSTONE), wire.below(), Direction::Up);
    assert!(Direction::PLANE_HORIZONTAL.iter().all(|&d| level.block_state(wire).get(super::redstone_wire_block::property(d)) == RedstoneSide::Side));
    click(&mut level, player, None, wire, Direction::Up);
    assert!(Direction::PLANE_HORIZONTAL.iter().all(|&d| level.block_state(wire).get(super::redstone_wire_block::property(d)) == RedstoneSide::None));
}

#[test]
fn repeater_delays_cycles_and_locks() {
    let (mut level, player) = setup();
    let repeater = at(0, 0);
    let lamp = at(0, -1);
    set(&mut level, lamp, blocks::REDSTONE_LAMP.default_state());
    click(&mut level, player, Some(items::REPEATER), repeater.below(), Direction::Up);
    let state = level.block_state(repeater);
    assert_eq!(state.get(HORIZONTAL_FACING), Direction::South);
    assert!(!state.get(LOCKED));

    click(&mut level, player, None, repeater, Direction::Up);
    assert_eq!(level.block_state(repeater).get(DELAY), 2);

    set(&mut level, at(0, 1), blocks::REDSTONE_BLOCK.default_state());
    ticks(&mut level, 3);
    assert!(!level.block_state(repeater).get(POWERED));
    ticks(&mut level, 1);
    assert!(level.block_state(repeater).get(POWERED));
    assert!(level.block_state(lamp).get(LIT));

    let side = blocks::REPEATER.default_state().set(HORIZONTAL_FACING, Direction::East).set(POWERED, true);
    set(&mut level, at(1, 0), side);
    assert!(level.block_state(repeater).get(LOCKED));
}

#[test]
fn observer_sends_a_pulse_out_of_its_back() {
    let (mut level, _) = setup();
    let observer = at(0, 0);
    let lamp = at(-1, 0);
    set(&mut level, lamp, blocks::REDSTONE_LAMP.default_state());
    set(&mut level, observer, blocks::OBSERVER.default_state().set(FACING, Direction::East));
    set(&mut level, at(1, 0), blocks::STONE.default_state());
    ticks(&mut level, 2);
    assert!(level.block_state(observer).get(POWERED));
    assert!(level.block_state(lamp).get(LIT));
    ticks(&mut level, 2);
    assert!(!level.block_state(observer).get(POWERED));
}

#[test]
fn note_block_takes_the_instrument_below_and_tunes_on_use() {
    let (mut level, player) = setup();
    let stone = at(0, 0);
    set(&mut level, stone, blocks::STONE.default_state());
    click(&mut level, player, Some(items::NOTE_BLOCK), stone, Direction::Up);
    let note_block = stone.above();
    assert_eq!(level.block_state(note_block).get(NOTEBLOCK_INSTRUMENT), NoteBlockInstrument::Basedrum);
    click(&mut level, player, None, note_block, Direction::North);
    assert_eq!(level.block_state(note_block).get(NOTE), 1);
    set(&mut level, stone, blocks::GOLD_BLOCK.default_state());
    assert_eq!(level.block_state(note_block).get(NOTEBLOCK_INSTRUMENT), NoteBlockInstrument::Bell);
    assert!((super::note_block::get_pitch_from_note(12) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn rails_curve_towards_a_third_rail() {
    let (mut level, _) = setup();
    let rail = blocks::RAIL.default_state();
    set(&mut level, at(0, 0), rail);
    set(&mut level, at(1, 0), rail);
    assert_eq!(level.block_state(at(0, 0)).get(RAIL_SHAPE), RailShape::EastWest);
    assert_eq!(level.block_state(at(1, 0)).get(RAIL_SHAPE), RailShape::EastWest);
    set(&mut level, at(0, 1), rail);
    assert_eq!(level.block_state(at(0, 0)).get(RAIL_SHAPE), RailShape::SouthEast);
    assert_eq!(level.block_state(at(0, 1)).get(RAIL_SHAPE), RailShape::NorthSouth);

    set(&mut level, at(2, 0), blocks::STONE.default_state());
    set(&mut level, at(2, 0).above(), rail);
    assert_eq!(level.block_state(at(1, 0)).get(RAIL_SHAPE), RailShape::AscendingEast);
}

#[test]
fn powered_rails_pass_power_along_the_line() {
    let (mut level, _) = setup();
    let rail = blocks::POWERED_RAIL.default_state().set(RAIL_SHAPE_STRAIGHT, RailShape::EastWest);
    for x in 0..10 {
        set(&mut level, at(x, 0), rail);
    }
    set(&mut level, at(-1, 0), blocks::REDSTONE_BLOCK.default_state());
    let powered: Vec<bool> = (0..10).map(|x| level.block_state(at(x, 0)).get(POWERED)).collect();
    assert_eq!(powered, [true, true, true, true, true, true, true, true, true, false]);
}

#[test]
fn pressure_plates_count_entities_on_them() {
    let (mut level, player) = setup();
    let plate = at(0, 0);
    set(&mut level, plate, blocks::STONE_PRESSURE_PLATE.default_state());
    level.player_mut(player).entity.set_pos(Vec3::new(0.5, GROUND as f64, 0.5));
    let state = level.block_state(plate);
    super::of(state).entity_inside(state, &mut level, plate, player, &mut InsideBlockEffects::default(), true);
    assert!(level.block_state(plate).get(POWERED));
    assert_eq!(level.get_direct_signal_to(plate.below()), 15);
    level.player_mut(player).entity.set_pos(Vec3::new(12.5, GROUND as f64, 12.5));
    ticks(&mut level, 20);
    assert!(!level.block_state(plate).get(POWERED));

    let weighted = at(2, 0);
    set(&mut level, weighted, blocks::LIGHT_WEIGHTED_PRESSURE_PLATE.default_state());
    for _ in 0..3 {
        let item = ItemEntity::new(Vec3::new(2.5, GROUND as f64, 0.5), ItemStack::new(items::STONE, 1));
        level.add_fresh_entity(LevelEntity::Item(item));
    }
    let state = level.block_state(weighted);
    super::of(state).entity_inside(state, &mut level, weighted, player, &mut InsideBlockEffects::default(), true);
    assert_eq!(level.block_state(weighted).get(POWER), 3);
}

#[test]
fn tripwire_hooks_attach_and_power_when_the_wire_is_pressed() {
    let (mut level, player) = setup();
    set(&mut level, at(0, -1), blocks::STONE.default_state());
    set(&mut level, at(0, 3), blocks::STONE.default_state());
    let hook = blocks::TRIPWIRE_HOOK.default_state();
    set(&mut level, at(0, 0), hook.set(HORIZONTAL_FACING, Direction::South));
    set(&mut level, at(0, 2), hook.set(HORIZONTAL_FACING, Direction::North));
    set(&mut level, at(0, 1), blocks::TRIPWIRE.default_state().set(NORTH, true).set(SOUTH, true));
    assert!(level.block_state(at(0, 0)).get(ATTACHED));
    assert!(level.block_state(at(0, 2)).get(ATTACHED));

    level.player_mut(player).entity.set_pos(Vec3::new(0.5, GROUND as f64, 1.5));
    let wire = level.block_state(at(0, 1));
    super::of(wire).entity_inside(wire, &mut level, at(0, 1), player, &mut InsideBlockEffects::default(), true);
    assert!(level.block_state(at(0, 1)).get(POWERED));
    assert!(level.block_state(at(0, 0)).get(POWERED));
    assert!(level.block_state(at(0, 2)).get(POWERED));
    assert_eq!(level.get_direct_signal_to(at(0, -1)), 15);

    level.player_mut(player).entity.set_pos(Vec3::new(12.5, GROUND as f64, 12.5));
    ticks(&mut level, 10);
    assert!(!level.block_state(at(0, 1)).get(POWERED));
    ticks(&mut level, 10);
    assert!(!level.block_state(at(0, 0)).get(POWERED));
    assert!(level.block_state(at(0, 0)).get(ATTACHED));
}
