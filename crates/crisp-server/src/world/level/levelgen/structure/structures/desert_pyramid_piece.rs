use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::LevelAccessor;
use crate::world::level::block;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::scattered_feature_piece::ScatteredFeaturePiece;
use crate::world::level::levelgen::structure::structure_piece::get_random_horizontal_direction;

pub const DESERT_PYRAMID_PIECE: &str = "minecraft:tedp";
const DESERT_PYRAMID: &str = "minecraft:chests/desert_pyramid";

#[derive(Clone)]
pub struct DesertPyramidPiece {
    pub scattered: ScatteredFeaturePiece,
    pub has_placed_chest: [bool; 4],
    pub potential_suspicious_sand_world_positions: Vec<BlockPos>,
    pub random_collapsed_roof_pos: BlockPos,
}

impl DesertPyramidPiece {
    pub fn new(random: &mut impl RandomSource, west: i32, north: i32) -> DesertPyramidPiece {
        DesertPyramidPiece {
            scattered: ScatteredFeaturePiece::new(DESERT_PYRAMID_PIECE, west, 64, north, 21, 15, 21, get_random_horizontal_direction(random)),
            has_placed_chest: [false; 4],
            potential_suspicious_sand_world_positions: Vec::new(),
            random_collapsed_roof_pos: BlockPos::ZERO,
        }
    }

    pub fn load(tag: &Nbt) -> Option<DesertPyramidPiece> {
        let flag = |key: &str| matches!(tag.get(key), Some(Nbt::Byte(1..)));
        Some(DesertPyramidPiece {
            scattered: ScatteredFeaturePiece::load(DESERT_PYRAMID_PIECE, tag)?,
            has_placed_chest: [flag("hasPlacedChest0"), flag("hasPlacedChest1"), flag("hasPlacedChest2"), flag("hasPlacedChest3")],
            potential_suspicious_sand_world_positions: Vec::new(),
            random_collapsed_roof_pos: BlockPos::ZERO,
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.scattered.add_additional_save_data(tag);
        for (i, placed) in self.has_placed_chest.iter().enumerate() {
            tag.push((format!("hasPlacedChest{i}"), Nbt::Byte(*placed as i8)));
        }
    }

    pub fn post_process(&mut self, level: &mut WorldGenRegion, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let offset = -random.next_int_bounded(3);
        if !self.scattered.update_height_position_to_lowest_ground_height(level, offset) {
            return;
        }
        let piece = &self.scattered.piece;
        let (width, depth) = (self.scattered.width, self.scattered.depth);
        let sandstone = blocks::SANDSTONE.default_state();
        let cut_sandstone = blocks::CUT_SANDSTONE.default_state();
        let chiseled_sandstone = blocks::CHISELED_SANDSTONE.default_state();
        let orange_terracotta = blocks::ORANGE_TERRACOTTA.default_state();
        let air = blocks::AIR.default_state();
        piece.generate_box(level, chunk_bb, 0, -4, 0, width - 1, 0, depth - 1, sandstone, sandstone, false);
        for pos in 1..=9 {
            piece.generate_box(level, chunk_bb, pos, pos, pos, width - 1 - pos, pos, depth - 1 - pos, sandstone, sandstone, false);
            piece.generate_box(level, chunk_bb, pos + 1, pos, pos + 1, width - 2 - pos, pos, depth - 2 - pos, air, air, false);
        }
        for x in 0..width {
            for z in 0..depth {
                piece.fill_column_down(level, sandstone, x, -5, z, chunk_bb);
            }
        }
        let stairs = |direction: Direction| -> BlockState { blocks::SANDSTONE_STAIRS.default_state().set(HORIZONTAL_FACING, direction) };
        let north_stairs = stairs(Direction::North);
        let south_stairs = stairs(Direction::South);
        let east_stairs = stairs(Direction::East);
        let west_stairs = stairs(Direction::West);
        piece.generate_box(level, chunk_bb, 0, 0, 0, 4, 9, 4, sandstone, air, false);
        piece.generate_box(level, chunk_bb, 1, 10, 1, 3, 10, 3, sandstone, sandstone, false);
        piece.place_block(level, north_stairs, 2, 10, 0, chunk_bb);
        piece.place_block(level, south_stairs, 2, 10, 4, chunk_bb);
        piece.place_block(level, east_stairs, 0, 10, 2, chunk_bb);
        piece.place_block(level, west_stairs, 4, 10, 2, chunk_bb);
        piece.generate_box(level, chunk_bb, width - 5, 0, 0, width - 1, 9, 4, sandstone, air, false);
        piece.generate_box(level, chunk_bb, width - 4, 10, 1, width - 2, 10, 3, sandstone, sandstone, false);
        piece.place_block(level, north_stairs, width - 3, 10, 0, chunk_bb);
        piece.place_block(level, south_stairs, width - 3, 10, 4, chunk_bb);
        piece.place_block(level, east_stairs, width - 5, 10, 2, chunk_bb);
        piece.place_block(level, west_stairs, width - 1, 10, 2, chunk_bb);
        piece.generate_box(level, chunk_bb, 8, 0, 0, 12, 4, 4, sandstone, air, false);
        piece.generate_box(level, chunk_bb, 9, 1, 0, 11, 3, 4, air, air, false);
        piece.place_block(level, cut_sandstone, 9, 1, 1, chunk_bb);
        piece.place_block(level, cut_sandstone, 9, 2, 1, chunk_bb);
        piece.place_block(level, cut_sandstone, 9, 3, 1, chunk_bb);
        piece.place_block(level, cut_sandstone, 10, 3, 1, chunk_bb);
        piece.place_block(level, cut_sandstone, 11, 3, 1, chunk_bb);
        piece.place_block(level, cut_sandstone, 11, 2, 1, chunk_bb);
        piece.place_block(level, cut_sandstone, 11, 1, 1, chunk_bb);
        piece.generate_box(level, chunk_bb, 4, 1, 1, 8, 3, 3, sandstone, air, false);
        piece.generate_box(level, chunk_bb, 4, 1, 2, 8, 2, 2, air, air, false);
        piece.generate_box(level, chunk_bb, 12, 1, 1, 16, 3, 3, sandstone, air, false);
        piece.generate_box(level, chunk_bb, 12, 1, 2, 16, 2, 2, air, air, false);
        piece.generate_box(level, chunk_bb, 5, 4, 5, width - 6, 4, depth - 6, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, 9, 4, 9, 11, 4, 11, air, air, false);
        piece.generate_box(level, chunk_bb, 8, 1, 8, 8, 3, 8, cut_sandstone, cut_sandstone, false);
        piece.generate_box(level, chunk_bb, 12, 1, 8, 12, 3, 8, cut_sandstone, cut_sandstone, false);
        piece.generate_box(level, chunk_bb, 8, 1, 12, 8, 3, 12, cut_sandstone, cut_sandstone, false);
        piece.generate_box(level, chunk_bb, 12, 1, 12, 12, 3, 12, cut_sandstone, cut_sandstone, false);
        piece.generate_box(level, chunk_bb, 1, 1, 5, 4, 4, 11, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, width - 5, 1, 5, width - 2, 4, 11, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, 6, 7, 9, 6, 7, 11, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, width - 7, 7, 9, width - 7, 7, 11, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, 5, 5, 9, 5, 7, 11, cut_sandstone, cut_sandstone, false);
        piece.generate_box(level, chunk_bb, width - 6, 5, 9, width - 6, 7, 11, cut_sandstone, cut_sandstone, false);
        piece.place_block(level, air, 5, 5, 10, chunk_bb);
        piece.place_block(level, air, 5, 6, 10, chunk_bb);
        piece.place_block(level, air, 6, 6, 10, chunk_bb);
        piece.place_block(level, air, width - 6, 5, 10, chunk_bb);
        piece.place_block(level, air, width - 6, 6, 10, chunk_bb);
        piece.place_block(level, air, width - 7, 6, 10, chunk_bb);
        piece.generate_box(level, chunk_bb, 2, 4, 4, 2, 6, 4, air, air, false);
        piece.generate_box(level, chunk_bb, width - 3, 4, 4, width - 3, 6, 4, air, air, false);
        piece.place_block(level, north_stairs, 2, 4, 5, chunk_bb);
        piece.place_block(level, north_stairs, 2, 3, 4, chunk_bb);
        piece.place_block(level, north_stairs, width - 3, 4, 5, chunk_bb);
        piece.place_block(level, north_stairs, width - 3, 3, 4, chunk_bb);
        piece.generate_box(level, chunk_bb, 1, 1, 3, 2, 2, 3, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, width - 3, 1, 3, width - 2, 2, 3, sandstone, sandstone, false);
        piece.place_block(level, sandstone, 1, 1, 2, chunk_bb);
        piece.place_block(level, sandstone, width - 2, 1, 2, chunk_bb);
        piece.place_block(level, blocks::SANDSTONE_SLAB.default_state(), 1, 2, 2, chunk_bb);
        piece.place_block(level, blocks::SANDSTONE_SLAB.default_state(), width - 2, 2, 2, chunk_bb);
        piece.place_block(level, west_stairs, 2, 1, 2, chunk_bb);
        piece.place_block(level, east_stairs, width - 3, 1, 2, chunk_bb);
        piece.generate_box(level, chunk_bb, 4, 3, 5, 4, 3, 17, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, width - 5, 3, 5, width - 5, 3, 17, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, 3, 1, 5, 4, 2, 16, air, air, false);
        piece.generate_box(level, chunk_bb, width - 6, 1, 5, width - 5, 2, 16, air, air, false);
        for z in (5..=17).step_by(2) {
            piece.place_block(level, cut_sandstone, 4, 1, z, chunk_bb);
            piece.place_block(level, chiseled_sandstone, 4, 2, z, chunk_bb);
            piece.place_block(level, cut_sandstone, width - 5, 1, z, chunk_bb);
            piece.place_block(level, chiseled_sandstone, width - 5, 2, z, chunk_bb);
        }
        piece.place_block(level, orange_terracotta, 10, 0, 7, chunk_bb);
        piece.place_block(level, orange_terracotta, 10, 0, 8, chunk_bb);
        piece.place_block(level, orange_terracotta, 9, 0, 9, chunk_bb);
        piece.place_block(level, orange_terracotta, 11, 0, 9, chunk_bb);
        piece.place_block(level, orange_terracotta, 8, 0, 10, chunk_bb);
        piece.place_block(level, orange_terracotta, 12, 0, 10, chunk_bb);
        piece.place_block(level, orange_terracotta, 7, 0, 10, chunk_bb);
        piece.place_block(level, orange_terracotta, 13, 0, 10, chunk_bb);
        piece.place_block(level, orange_terracotta, 9, 0, 11, chunk_bb);
        piece.place_block(level, orange_terracotta, 11, 0, 11, chunk_bb);
        piece.place_block(level, orange_terracotta, 10, 0, 12, chunk_bb);
        piece.place_block(level, orange_terracotta, 10, 0, 13, chunk_bb);
        piece.place_block(level, blocks::BLUE_TERRACOTTA.default_state(), 10, 0, 10, chunk_bb);
        for x in (0..=width - 1).step_by((width - 1) as usize) {
            piece.place_block(level, cut_sandstone, x, 2, 1, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 2, 2, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 2, 3, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 3, 1, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 3, 2, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 3, 3, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 4, 1, chunk_bb);
            piece.place_block(level, chiseled_sandstone, x, 4, 2, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 4, 3, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 5, 1, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 5, 2, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 5, 3, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 6, 1, chunk_bb);
            piece.place_block(level, chiseled_sandstone, x, 6, 2, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 6, 3, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 7, 1, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 7, 2, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 7, 3, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 8, 1, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 8, 2, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 8, 3, chunk_bb);
        }
        for x in (2..=width - 3).step_by((width - 3 - 2) as usize) {
            piece.place_block(level, cut_sandstone, x - 1, 2, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 2, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x + 1, 2, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x - 1, 3, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 3, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x + 1, 3, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x - 1, 4, 0, chunk_bb);
            piece.place_block(level, chiseled_sandstone, x, 4, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x + 1, 4, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x - 1, 5, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 5, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x + 1, 5, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x - 1, 6, 0, chunk_bb);
            piece.place_block(level, chiseled_sandstone, x, 6, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x + 1, 6, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x - 1, 7, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x, 7, 0, chunk_bb);
            piece.place_block(level, orange_terracotta, x + 1, 7, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x - 1, 8, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x, 8, 0, chunk_bb);
            piece.place_block(level, cut_sandstone, x + 1, 8, 0, chunk_bb);
        }
        piece.generate_box(level, chunk_bb, 8, 4, 0, 12, 6, 0, cut_sandstone, cut_sandstone, false);
        piece.place_block(level, air, 8, 6, 0, chunk_bb);
        piece.place_block(level, air, 12, 6, 0, chunk_bb);
        piece.place_block(level, orange_terracotta, 9, 5, 0, chunk_bb);
        piece.place_block(level, chiseled_sandstone, 10, 5, 0, chunk_bb);
        piece.place_block(level, orange_terracotta, 11, 5, 0, chunk_bb);
        piece.generate_box(level, chunk_bb, 8, -14, 8, 12, -11, 12, cut_sandstone, cut_sandstone, false);
        piece.generate_box(level, chunk_bb, 8, -10, 8, 12, -10, 12, chiseled_sandstone, chiseled_sandstone, false);
        piece.generate_box(level, chunk_bb, 8, -9, 8, 12, -9, 12, cut_sandstone, cut_sandstone, false);
        piece.generate_box(level, chunk_bb, 8, -8, 8, 12, -1, 12, sandstone, sandstone, false);
        piece.generate_box(level, chunk_bb, 9, -11, 9, 11, -1, 11, air, air, false);
        piece.place_block(level, blocks::STONE_PRESSURE_PLATE.default_state(), 10, -11, 10, chunk_bb);
        piece.generate_box(level, chunk_bb, 9, -13, 9, 11, -13, 11, blocks::TNT.default_state(), air, false);
        piece.place_block(level, air, 8, -11, 10, chunk_bb);
        piece.place_block(level, air, 8, -10, 10, chunk_bb);
        piece.place_block(level, chiseled_sandstone, 7, -10, 10, chunk_bb);
        piece.place_block(level, cut_sandstone, 7, -11, 10, chunk_bb);
        piece.place_block(level, air, 12, -11, 10, chunk_bb);
        piece.place_block(level, air, 12, -10, 10, chunk_bb);
        piece.place_block(level, chiseled_sandstone, 13, -10, 10, chunk_bb);
        piece.place_block(level, cut_sandstone, 13, -11, 10, chunk_bb);
        piece.place_block(level, air, 10, -11, 8, chunk_bb);
        piece.place_block(level, air, 10, -10, 8, chunk_bb);
        piece.place_block(level, chiseled_sandstone, 10, -10, 7, chunk_bb);
        piece.place_block(level, cut_sandstone, 10, -11, 7, chunk_bb);
        piece.place_block(level, air, 10, -11, 12, chunk_bb);
        piece.place_block(level, air, 10, -10, 12, chunk_bb);
        piece.place_block(level, chiseled_sandstone, 10, -10, 13, chunk_bb);
        piece.place_block(level, cut_sandstone, 10, -11, 13, chunk_bb);
        for direction in Direction::PLANE_HORIZONTAL {
            let index = direction.get_2d_data_value() as usize;
            if !self.has_placed_chest[index] {
                let xo = direction.step_x() * 2;
                let zo = direction.step_z() * 2;
                self.has_placed_chest[index] = piece.create_chest(level, chunk_bb, random, piece.get_world_pos(10 + xo, -11, 10 + zo), DESERT_PYRAMID, None);
            }
        }
        self.add_cellar(level, chunk_bb);
    }

    fn add_cellar(&mut self, level: &mut WorldGenRegion, chunk_bb: &BoundingBox) {
        let room_center = BlockPos::new(16, -4, 13);
        self.add_cellar_stairs(room_center, level, chunk_bb);
        self.add_cellar_room(room_center, level, chunk_bb);
    }

    fn add_cellar_stairs(&self, room_center: BlockPos, level: &mut WorldGenRegion, chunk_bb: &BoundingBox) {
        let piece = &self.scattered.piece;
        let (x, y, z) = (room_center.x, room_center.y, room_center.z);
        let sand_stone_stairs = blocks::SANDSTONE_STAIRS.default_state();
        piece.place_block(level, block::rotate(sand_stone_stairs, Rotation::Counterclockwise90), 13, -1, 17, chunk_bb);
        piece.place_block(level, block::rotate(sand_stone_stairs, Rotation::Counterclockwise90), 14, -2, 17, chunk_bb);
        piece.place_block(level, block::rotate(sand_stone_stairs, Rotation::Counterclockwise90), 15, -3, 17, chunk_bb);
        let sand = blocks::SAND.default_state();
        let sand_stone = blocks::SANDSTONE.default_state();
        let variant = level.get_random().next_boolean();
        piece.place_block(level, sand, x - 4, y + 4, z + 4, chunk_bb);
        piece.place_block(level, sand, x - 3, y + 4, z + 4, chunk_bb);
        piece.place_block(level, sand, x - 2, y + 4, z + 4, chunk_bb);
        piece.place_block(level, sand, x - 1, y + 4, z + 4, chunk_bb);
        piece.place_block(level, sand, x, y + 4, z + 4, chunk_bb);
        piece.place_block(level, sand, x - 2, y + 3, z + 4, chunk_bb);
        piece.place_block(level, if variant { sand } else { sand_stone }, x - 1, y + 3, z + 4, chunk_bb);
        piece.place_block(level, if !variant { sand } else { sand_stone }, x, y + 3, z + 4, chunk_bb);
        piece.place_block(level, sand, x - 1, y + 2, z + 4, chunk_bb);
        piece.place_block(level, sand_stone, x, y + 2, z + 4, chunk_bb);
        piece.place_block(level, sand, x, y + 1, z + 4, chunk_bb);
    }

    fn add_cellar_room(&mut self, room_center: BlockPos, level: &mut WorldGenRegion, chunk_bb: &BoundingBox) {
        let (x, y, z) = (room_center.x, room_center.y, room_center.z);
        let cut_sand_stone = blocks::CUT_SANDSTONE.default_state();
        let hieroglyphs_sand_stone = blocks::CHISELED_SANDSTONE.default_state();
        let piece = &self.scattered.piece;
        piece.generate_box(level, chunk_bb, x - 3, y + 1, z - 3, x - 3, y + 1, z + 2, cut_sand_stone, cut_sand_stone, true);
        piece.generate_box(level, chunk_bb, x + 3, y + 1, z - 3, x + 3, y + 1, z + 2, cut_sand_stone, cut_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, y + 1, z - 3, x + 3, y + 1, z - 2, cut_sand_stone, cut_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, y + 1, z + 3, x + 3, y + 1, z + 3, cut_sand_stone, cut_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, y + 2, z - 3, x - 3, y + 2, z + 2, hieroglyphs_sand_stone, hieroglyphs_sand_stone, true);
        piece.generate_box(level, chunk_bb, x + 3, y + 2, z - 3, x + 3, y + 2, z + 2, hieroglyphs_sand_stone, hieroglyphs_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, y + 2, z - 3, x + 3, y + 2, z - 2, hieroglyphs_sand_stone, hieroglyphs_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, y + 2, z + 3, x + 3, y + 2, z + 3, hieroglyphs_sand_stone, hieroglyphs_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, -1, z - 3, x - 3, -1, z + 2, cut_sand_stone, cut_sand_stone, true);
        piece.generate_box(level, chunk_bb, x + 3, -1, z - 3, x + 3, -1, z + 2, cut_sand_stone, cut_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, -1, z - 3, x + 3, -1, z - 2, cut_sand_stone, cut_sand_stone, true);
        piece.generate_box(level, chunk_bb, x - 3, -1, z + 3, x + 3, -1, z + 3, cut_sand_stone, cut_sand_stone, true);
        self.place_sand_box(x - 2, y + 1, z - 2, x + 2, y + 3, z + 2);
        self.place_collapsed_roof(level, chunk_bb, x - 2, y + 4, z - 2, x + 2, z + 2);
        let piece = &self.scattered.piece;
        let orange_teracotta = blocks::ORANGE_TERRACOTTA.default_state();
        let blue_teracotta = blocks::BLUE_TERRACOTTA.default_state();
        piece.place_block(level, blue_teracotta, x, y, z, chunk_bb);
        piece.place_block(level, orange_teracotta, x + 1, y, z - 1, chunk_bb);
        piece.place_block(level, orange_teracotta, x + 1, y, z + 1, chunk_bb);
        piece.place_block(level, orange_teracotta, x - 1, y, z - 1, chunk_bb);
        piece.place_block(level, orange_teracotta, x - 1, y, z + 1, chunk_bb);
        piece.place_block(level, orange_teracotta, x + 2, y, z, chunk_bb);
        piece.place_block(level, orange_teracotta, x - 2, y, z, chunk_bb);
        piece.place_block(level, orange_teracotta, x, y, z + 2, chunk_bb);
        piece.place_block(level, orange_teracotta, x, y, z - 2, chunk_bb);
        piece.place_block(level, orange_teracotta, x + 3, y, z, chunk_bb);
        self.place_sand(x + 3, y + 1, z);
        self.place_sand(x + 3, y + 2, z);
        let piece = &self.scattered.piece;
        piece.place_block(level, cut_sand_stone, x + 4, y + 1, z, chunk_bb);
        piece.place_block(level, hieroglyphs_sand_stone, x + 4, y + 2, z, chunk_bb);
        piece.place_block(level, orange_teracotta, x - 3, y, z, chunk_bb);
        self.place_sand(x - 3, y + 1, z);
        self.place_sand(x - 3, y + 2, z);
        let piece = &self.scattered.piece;
        piece.place_block(level, cut_sand_stone, x - 4, y + 1, z, chunk_bb);
        piece.place_block(level, hieroglyphs_sand_stone, x - 4, y + 2, z, chunk_bb);
        piece.place_block(level, orange_teracotta, x, y, z + 3, chunk_bb);
        self.place_sand(x, y + 1, z + 3);
        self.place_sand(x, y + 2, z + 3);
        self.scattered.piece.place_block(level, orange_teracotta, x, y, z - 3, chunk_bb);
        self.place_sand(x, y + 1, z - 3);
        self.place_sand(x, y + 2, z - 3);
        let piece = &self.scattered.piece;
        piece.place_block(level, cut_sand_stone, x, y + 1, z - 4, chunk_bb);
        piece.place_block(level, hieroglyphs_sand_stone, x, -2, z - 4, chunk_bb);
    }

    fn place_sand(&mut self, x: i32, y: i32, z: i32) {
        let world_pos = self.scattered.piece.get_world_pos(x, y, z);
        self.potential_suspicious_sand_world_positions.push(world_pos);
    }

    fn place_sand_box(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    self.place_sand(x, y, z);
                }
            }
        }
    }

    fn place_collapsed_roof_piece(&self, level: &mut WorldGenRegion, x: i32, y: i32, z: i32, chunk_bb: &BoundingBox) {
        if level.get_random().next_float() < 0.33 {
            self.scattered.piece.place_block(level, blocks::SANDSTONE.default_state(), x, y, z, chunk_bb);
        } else {
            self.scattered.piece.place_block(level, blocks::SAND.default_state(), x, y, z, chunk_bb);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_collapsed_roof(&mut self, level: &mut WorldGenRegion, chunk_bb: &BoundingBox, x0: i32, y0: i32, z0: i32, x1: i32, z1: i32) {
        for x in x0..=x1 {
            for z in z0..=z1 {
                self.place_collapsed_roof_piece(level, x, y0, z, chunk_bb);
            }
        }
        let piece = &self.scattered.piece;
        let origin = piece.get_world_pos(x0, y0, z0);
        let mut random = LegacyRandomSource::new(level.get_seed()).fork_positional().at(origin.x, origin.y, origin.z);
        let roof_pos_x = random.next_int_between_inclusive(x0, x1);
        let roof_pos_z = random.next_int_between_inclusive(z0, z1);
        self.random_collapsed_roof_pos = BlockPos::new(piece.get_world_x(roof_pos_x, roof_pos_z), piece.get_world_y(y0), piece.get_world_z(roof_pos_x, roof_pos_z));
    }

    pub fn get_potential_suspicious_sand_world_positions(&self) -> &[BlockPos] {
        &self.potential_suspicious_sand_world_positions
    }

    pub fn get_random_collapsed_roof_pos(&self) -> BlockPos {
        self.random_collapsed_roof_pos
    }
}
