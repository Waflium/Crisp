use crisp_core::Direction;
use crisp_core::random::RandomSource;
use crisp_data::properties::{
    ATTACH_FACE, ATTACHED, AttachFace, EAST, EAST_REDSTONE, FACING, HORIZONTAL_FACING, NORTH, NORTH_REDSTONE, RedstoneSide, SOUTH, SOUTH_REDSTONE, WEST, WEST_REDSTONE,
};
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::scattered_feature_piece::ScatteredFeaturePiece;
use crate::world::level::levelgen::structure::structure_piece::{BlockSelector, get_random_horizontal_direction};

pub const JUNGLE_PYRAMID_PIECE: &str = "minecraft:tejp";
const JUNGLE_TEMPLE: &str = "minecraft:chests/jungle_temple";
const JUNGLE_TEMPLE_DISPENSER: &str = "minecraft:chests/jungle_temple_dispenser";

#[derive(Clone)]
pub struct JungleTemplePiece {
    pub scattered: ScatteredFeaturePiece,
    pub placed_main_chest: bool,
    pub placed_hidden_chest: bool,
    pub placed_trap1: bool,
    pub placed_trap2: bool,
}

impl JungleTemplePiece {
    pub fn new(random: &mut impl RandomSource, west: i32, north: i32) -> JungleTemplePiece {
        JungleTemplePiece {
            scattered: ScatteredFeaturePiece::new(JUNGLE_PYRAMID_PIECE, west, 64, north, 12, 10, 15, get_random_horizontal_direction(random)),
            placed_main_chest: false,
            placed_hidden_chest: false,
            placed_trap1: false,
            placed_trap2: false,
        }
    }

    pub fn load(tag: &Nbt) -> Option<JungleTemplePiece> {
        let flag = |key: &str| matches!(tag.get(key), Some(Nbt::Byte(1..)));
        Some(JungleTemplePiece {
            scattered: ScatteredFeaturePiece::load(JUNGLE_PYRAMID_PIECE, tag)?,
            placed_main_chest: flag("placedMainChest"),
            placed_hidden_chest: flag("placedHiddenChest"),
            placed_trap1: flag("placedTrap1"),
            placed_trap2: flag("placedTrap2"),
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.scattered.add_additional_save_data(tag);
        tag.push(("placedMainChest".into(), Nbt::Byte(self.placed_main_chest as i8)));
        tag.push(("placedHiddenChest".into(), Nbt::Byte(self.placed_hidden_chest as i8)));
        tag.push(("placedTrap1".into(), Nbt::Byte(self.placed_trap1 as i8)));
        tag.push(("placedTrap2".into(), Nbt::Byte(self.placed_trap2 as i8)));
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        if !self.scattered.update_average_ground_height(level, chunk_bb, 0) {
            return;
        }
        let piece = &self.scattered.piece;
        let (width, depth) = (self.scattered.width, self.scattered.depth);
        let stone_selector = &mut MossStoneSelector { next: blocks::AIR.default_state() };
        piece.generate_box_with_selector(level, chunk_bb, 0, -4, 0, width - 1, 0, depth - 1, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 1, 2, 9, 2, 2, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 1, 12, 9, 2, 12, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 1, 3, 2, 2, 11, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 9, 1, 3, 9, 2, 11, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 1, 3, 1, 10, 6, 1, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 1, 3, 13, 10, 6, 13, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 1, 3, 2, 1, 6, 12, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 10, 3, 2, 10, 6, 12, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 3, 2, 9, 3, 12, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 6, 2, 9, 6, 12, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 3, 7, 3, 8, 7, 11, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 8, 4, 7, 8, 10, false, random, stone_selector);
        piece.generate_air_box(level, chunk_bb, 3, 1, 3, 8, 2, 11);
        piece.generate_air_box(level, chunk_bb, 4, 3, 6, 7, 3, 9);
        piece.generate_air_box(level, chunk_bb, 2, 4, 2, 9, 5, 12);
        piece.generate_air_box(level, chunk_bb, 4, 6, 5, 7, 6, 9);
        piece.generate_air_box(level, chunk_bb, 5, 7, 6, 6, 7, 8);
        piece.generate_air_box(level, chunk_bb, 5, 1, 2, 6, 2, 2);
        piece.generate_air_box(level, chunk_bb, 5, 2, 12, 6, 2, 12);
        piece.generate_air_box(level, chunk_bb, 5, 5, 1, 6, 5, 1);
        piece.generate_air_box(level, chunk_bb, 5, 5, 13, 6, 5, 13);
        piece.place_block(level, blocks::AIR.default_state(), 1, 5, 5, chunk_bb);
        piece.place_block(level, blocks::AIR.default_state(), 10, 5, 5, chunk_bb);
        piece.place_block(level, blocks::AIR.default_state(), 1, 5, 9, chunk_bb);
        piece.place_block(level, blocks::AIR.default_state(), 10, 5, 9, chunk_bb);
        for z in [0, 14] {
            piece.generate_box_with_selector(level, chunk_bb, 2, 4, z, 2, 5, z, false, random, stone_selector);
            piece.generate_box_with_selector(level, chunk_bb, 4, 4, z, 4, 5, z, false, random, stone_selector);
            piece.generate_box_with_selector(level, chunk_bb, 7, 4, z, 7, 5, z, false, random, stone_selector);
            piece.generate_box_with_selector(level, chunk_bb, 9, 4, z, 9, 5, z, false, random, stone_selector);
        }
        piece.generate_box_with_selector(level, chunk_bb, 5, 6, 0, 6, 6, 0, false, random, stone_selector);
        for x in [0, 11] {
            for z in (2..=12).step_by(2) {
                piece.generate_box_with_selector(level, chunk_bb, x, 4, z, x, 5, z, false, random, stone_selector);
            }
            piece.generate_box_with_selector(level, chunk_bb, x, 6, 5, x, 6, 5, false, random, stone_selector);
            piece.generate_box_with_selector(level, chunk_bb, x, 6, 9, x, 6, 9, false, random, stone_selector);
        }
        piece.generate_box_with_selector(level, chunk_bb, 2, 7, 2, 2, 9, 2, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 9, 7, 2, 9, 9, 2, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 7, 12, 2, 9, 12, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 9, 7, 12, 9, 9, 12, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 9, 4, 4, 9, 4, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 7, 9, 4, 7, 9, 4, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 9, 10, 4, 9, 10, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 7, 9, 10, 7, 9, 10, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 5, 9, 7, 6, 9, 7, false, random, stone_selector);
        let stairs = |direction: Direction| -> BlockState { blocks::COBBLESTONE_STAIRS.default_state().set(HORIZONTAL_FACING, direction) };
        let east_stairs = stairs(Direction::East);
        let west_stairs = stairs(Direction::West);
        let south_stairs = stairs(Direction::South);
        let north_stairs = stairs(Direction::North);
        piece.place_block(level, north_stairs, 5, 9, 6, chunk_bb);
        piece.place_block(level, north_stairs, 6, 9, 6, chunk_bb);
        piece.place_block(level, south_stairs, 5, 9, 8, chunk_bb);
        piece.place_block(level, south_stairs, 6, 9, 8, chunk_bb);
        piece.place_block(level, north_stairs, 4, 0, 0, chunk_bb);
        piece.place_block(level, north_stairs, 5, 0, 0, chunk_bb);
        piece.place_block(level, north_stairs, 6, 0, 0, chunk_bb);
        piece.place_block(level, north_stairs, 7, 0, 0, chunk_bb);
        piece.place_block(level, north_stairs, 4, 1, 8, chunk_bb);
        piece.place_block(level, north_stairs, 4, 2, 9, chunk_bb);
        piece.place_block(level, north_stairs, 4, 3, 10, chunk_bb);
        piece.place_block(level, north_stairs, 7, 1, 8, chunk_bb);
        piece.place_block(level, north_stairs, 7, 2, 9, chunk_bb);
        piece.place_block(level, north_stairs, 7, 3, 10, chunk_bb);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 9, 4, 1, 9, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 7, 1, 9, 7, 1, 9, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 10, 7, 2, 10, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 5, 4, 5, 6, 4, 5, false, random, stone_selector);
        piece.place_block(level, east_stairs, 4, 4, 5, chunk_bb);
        piece.place_block(level, west_stairs, 7, 4, 5, chunk_bb);
        for i in 0..4 {
            piece.place_block(level, south_stairs, 5, -i, 6 + i, chunk_bb);
            piece.place_block(level, south_stairs, 6, -i, 6 + i, chunk_bb);
            piece.generate_air_box(level, chunk_bb, 5, -i, 7 + i, 6, -i, 9 + i);
        }
        piece.generate_air_box(level, chunk_bb, 1, -3, 12, 10, -1, 13);
        piece.generate_air_box(level, chunk_bb, 1, -3, 1, 3, -1, 13);
        piece.generate_air_box(level, chunk_bb, 1, -3, 1, 9, -1, 5);
        for z in (1..=13).step_by(2) {
            piece.generate_box_with_selector(level, chunk_bb, 1, -3, z, 1, -2, z, false, random, stone_selector);
        }
        for z in (2..=12).step_by(2) {
            piece.generate_box_with_selector(level, chunk_bb, 1, -1, z, 3, -1, z, false, random, stone_selector);
        }
        piece.generate_box_with_selector(level, chunk_bb, 2, -2, 1, 5, -2, 1, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 7, -2, 1, 9, -2, 1, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 6, -3, 1, 6, -3, 1, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 6, -1, 1, 6, -1, 1, false, random, stone_selector);
        let hook = |direction: Direction| -> BlockState { blocks::TRIPWIRE_HOOK.default_state().set(HORIZONTAL_FACING, direction).set(ATTACHED, true) };
        let tripwire_ew = blocks::TRIPWIRE.default_state().set(EAST, true).set(WEST, true).set(ATTACHED, true);
        let tripwire_ns = blocks::TRIPWIRE.default_state().set(NORTH, true).set(SOUTH, true).set(ATTACHED, true);
        piece.place_block(level, hook(Direction::East), 1, -3, 8, chunk_bb);
        piece.place_block(level, hook(Direction::West), 4, -3, 8, chunk_bb);
        piece.place_block(level, tripwire_ew, 2, -3, 8, chunk_bb);
        piece.place_block(level, tripwire_ew, 3, -3, 8, chunk_bb);
        let redstone_wire = blocks::REDSTONE_WIRE.default_state();
        let redstone_wire_ns = redstone_wire.set(NORTH_REDSTONE, RedstoneSide::Side).set(SOUTH_REDSTONE, RedstoneSide::Side);
        piece.place_block(level, redstone_wire_ns, 5, -3, 7, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 5, -3, 6, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 5, -3, 5, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 5, -3, 4, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 5, -3, 3, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 5, -3, 2, chunk_bb);
        piece.place_block(level, redstone_wire.set(NORTH_REDSTONE, RedstoneSide::Side).set(WEST_REDSTONE, RedstoneSide::Side), 5, -3, 1, chunk_bb);
        piece.place_block(level, redstone_wire.set(EAST_REDSTONE, RedstoneSide::Side).set(WEST_REDSTONE, RedstoneSide::Side), 4, -3, 1, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 3, -3, 1, chunk_bb);
        if !self.placed_trap1 {
            self.placed_trap1 = piece.create_dispenser(level, chunk_bb, random, 3, -2, 1, Direction::North, JUNGLE_TEMPLE_DISPENSER);
        }
        piece.place_block(level, blocks::VINE.default_state().set(SOUTH, true), 3, -2, 2, chunk_bb);
        piece.place_block(level, hook(Direction::North), 7, -3, 1, chunk_bb);
        piece.place_block(level, hook(Direction::South), 7, -3, 5, chunk_bb);
        piece.place_block(level, tripwire_ns, 7, -3, 2, chunk_bb);
        piece.place_block(level, tripwire_ns, 7, -3, 3, chunk_bb);
        piece.place_block(level, tripwire_ns, 7, -3, 4, chunk_bb);
        piece.place_block(level, redstone_wire.set(EAST_REDSTONE, RedstoneSide::Side).set(WEST_REDSTONE, RedstoneSide::Side), 8, -3, 6, chunk_bb);
        piece.place_block(level, redstone_wire.set(WEST_REDSTONE, RedstoneSide::Side).set(SOUTH_REDSTONE, RedstoneSide::Side), 9, -3, 6, chunk_bb);
        piece.place_block(level, redstone_wire.set(NORTH_REDSTONE, RedstoneSide::Side).set(SOUTH_REDSTONE, RedstoneSide::Up), 9, -3, 5, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 9, -3, 4, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 9, -2, 4, chunk_bb);
        if !self.placed_trap2 {
            self.placed_trap2 = piece.create_dispenser(level, chunk_bb, random, 9, -2, 3, Direction::West, JUNGLE_TEMPLE_DISPENSER);
        }
        piece.place_block(level, blocks::VINE.default_state().set(EAST, true), 8, -1, 3, chunk_bb);
        piece.place_block(level, blocks::VINE.default_state().set(EAST, true), 8, -2, 3, chunk_bb);
        if !self.placed_main_chest {
            self.placed_main_chest = piece.create_chest(level, chunk_bb, random, piece.get_world_pos(8, -3, 3), JUNGLE_TEMPLE, None);
        }
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 9, -3, 2, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 8, -3, 1, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 4, -3, 5, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 5, -2, 5, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 5, -1, 5, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 6, -3, 5, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 7, -2, 5, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 7, -1, 5, chunk_bb);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 8, -3, 5, chunk_bb);
        piece.generate_box_with_selector(level, chunk_bb, 9, -1, 1, 9, -1, 5, false, random, stone_selector);
        piece.generate_air_box(level, chunk_bb, 8, -3, 8, 10, -1, 10);
        piece.place_block(level, blocks::CHISELED_STONE_BRICKS.default_state(), 8, -2, 11, chunk_bb);
        piece.place_block(level, blocks::CHISELED_STONE_BRICKS.default_state(), 9, -2, 11, chunk_bb);
        piece.place_block(level, blocks::CHISELED_STONE_BRICKS.default_state(), 10, -2, 11, chunk_bb);
        let lever = blocks::LEVER.default_state().set(HORIZONTAL_FACING, Direction::North).set(ATTACH_FACE, AttachFace::Wall);
        piece.place_block(level, lever, 8, -2, 12, chunk_bb);
        piece.place_block(level, lever, 9, -2, 12, chunk_bb);
        piece.place_block(level, lever, 10, -2, 12, chunk_bb);
        piece.generate_box_with_selector(level, chunk_bb, 8, -3, 8, 8, -3, 10, false, random, stone_selector);
        piece.generate_box_with_selector(level, chunk_bb, 10, -3, 8, 10, -3, 10, false, random, stone_selector);
        piece.place_block(level, blocks::MOSSY_COBBLESTONE.default_state(), 10, -2, 9, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 8, -2, 9, chunk_bb);
        piece.place_block(level, redstone_wire_ns, 8, -2, 10, chunk_bb);
        piece.place_block(
            level,
            redstone_wire
                .set(NORTH_REDSTONE, RedstoneSide::Side)
                .set(SOUTH_REDSTONE, RedstoneSide::Side)
                .set(EAST_REDSTONE, RedstoneSide::Side)
                .set(WEST_REDSTONE, RedstoneSide::Side),
            10,
            -1,
            9,
            chunk_bb,
        );
        piece.place_block(level, blocks::STICKY_PISTON.default_state().set(FACING, Direction::Up), 9, -2, 8, chunk_bb);
        piece.place_block(level, blocks::STICKY_PISTON.default_state().set(FACING, Direction::West), 10, -2, 8, chunk_bb);
        piece.place_block(level, blocks::STICKY_PISTON.default_state().set(FACING, Direction::West), 10, -1, 8, chunk_bb);
        piece.place_block(level, blocks::REPEATER.default_state().set(HORIZONTAL_FACING, Direction::North), 10, -2, 10, chunk_bb);
        if !self.placed_hidden_chest {
            self.placed_hidden_chest = piece.create_chest(level, chunk_bb, random, piece.get_world_pos(9, -3, 10), JUNGLE_TEMPLE, None);
        }
    }
}

struct MossStoneSelector {
    next: BlockState,
}

impl BlockSelector for MossStoneSelector {
    fn next(&mut self, random: &mut impl RandomSource, _world_x: i32, _world_y: i32, _world_z: i32, _is_edge: bool) {
        self.next = if random.next_float() < 0.4 { blocks::COBBLESTONE.default_state() } else { blocks::MOSSY_COBBLESTONE.default_state() };
    }

    fn get_next(&self) -> BlockState {
        self.next
    }
}
