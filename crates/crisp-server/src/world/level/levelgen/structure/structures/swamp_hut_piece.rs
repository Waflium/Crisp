use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, ChunkPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, STAIRS_SHAPE, StairsShape};
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::scattered_feature_piece::ScatteredFeaturePiece;
use crate::world::level::levelgen::structure::structure_piece::get_random_horizontal_direction;

pub const SWAMPLAND_HUT: &str = "minecraft:tesh";

#[derive(Clone)]
pub struct SwampHutPiece {
    pub scattered: ScatteredFeaturePiece,
    spawned_witch: bool,
    spawned_cat: bool,
}

impl SwampHutPiece {
    pub fn new(random: &mut impl RandomSource, west: i32, north: i32) -> SwampHutPiece {
        SwampHutPiece {
            scattered: ScatteredFeaturePiece::new(SWAMPLAND_HUT, west, 64, north, 7, 7, 9, get_random_horizontal_direction(random)),
            spawned_witch: false,
            spawned_cat: false,
        }
    }

    pub fn load(tag: &Nbt) -> Option<SwampHutPiece> {
        Some(SwampHutPiece {
            scattered: ScatteredFeaturePiece::load(SWAMPLAND_HUT, tag)?,
            spawned_witch: matches!(tag.get("Witch"), Some(Nbt::Byte(1..))),
            spawned_cat: matches!(tag.get("Cat"), Some(Nbt::Byte(1..))),
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.scattered.add_additional_save_data(tag);
        tag.push(("Witch".into(), Nbt::Byte(self.spawned_witch as i8)));
        tag.push(("Cat".into(), Nbt::Byte(self.spawned_cat as i8)));
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, _chunk_pos: ChunkPos, _reference_pos: BlockPos) {
        if !self.scattered.update_average_ground_height(level, chunk_bb, 0) {
            return;
        }
        let piece = &self.scattered.piece;
        let planks = blocks::SPRUCE_PLANKS.default_state();
        let log = blocks::OAK_LOG.default_state();
        piece.generate_box(level, chunk_bb, 1, 1, 1, 5, 1, 7, planks, planks, false);
        piece.generate_box(level, chunk_bb, 1, 4, 2, 5, 4, 7, planks, planks, false);
        piece.generate_box(level, chunk_bb, 2, 1, 0, 4, 1, 0, planks, planks, false);
        piece.generate_box(level, chunk_bb, 2, 2, 2, 3, 3, 2, planks, planks, false);
        piece.generate_box(level, chunk_bb, 1, 2, 3, 1, 3, 6, planks, planks, false);
        piece.generate_box(level, chunk_bb, 5, 2, 3, 5, 3, 6, planks, planks, false);
        piece.generate_box(level, chunk_bb, 2, 2, 7, 4, 3, 7, planks, planks, false);
        piece.generate_box(level, chunk_bb, 1, 0, 2, 1, 3, 2, log, log, false);
        piece.generate_box(level, chunk_bb, 5, 0, 2, 5, 3, 2, log, log, false);
        piece.generate_box(level, chunk_bb, 1, 0, 7, 1, 3, 7, log, log, false);
        piece.generate_box(level, chunk_bb, 5, 0, 7, 5, 3, 7, log, log, false);
        piece.place_block(level, blocks::OAK_FENCE.default_state(), 2, 3, 2, chunk_bb);
        piece.place_block(level, blocks::OAK_FENCE.default_state(), 3, 3, 7, chunk_bb);
        piece.place_block(level, blocks::AIR.default_state(), 1, 3, 4, chunk_bb);
        piece.place_block(level, blocks::AIR.default_state(), 5, 3, 4, chunk_bb);
        piece.place_block(level, blocks::AIR.default_state(), 5, 3, 5, chunk_bb);
        piece.place_block(level, blocks::POTTED_RED_MUSHROOM.default_state(), 1, 3, 5, chunk_bb);
        piece.place_block(level, blocks::CRAFTING_TABLE.default_state(), 3, 2, 6, chunk_bb);
        piece.place_block(level, blocks::CAULDRON.default_state(), 4, 2, 6, chunk_bb);
        piece.place_block(level, blocks::OAK_FENCE.default_state(), 1, 2, 1, chunk_bb);
        piece.place_block(level, blocks::OAK_FENCE.default_state(), 5, 2, 1, chunk_bb);
        let stairs = |direction: Direction| -> BlockState { blocks::SPRUCE_STAIRS.default_state().set(HORIZONTAL_FACING, direction) };
        let north_stairs = stairs(Direction::North);
        let east_stairs = stairs(Direction::East);
        let west_stairs = stairs(Direction::West);
        let south_stairs = stairs(Direction::South);
        piece.generate_box(level, chunk_bb, 0, 4, 1, 6, 4, 1, north_stairs, north_stairs, false);
        piece.generate_box(level, chunk_bb, 0, 4, 2, 0, 4, 7, east_stairs, east_stairs, false);
        piece.generate_box(level, chunk_bb, 6, 4, 2, 6, 4, 7, west_stairs, west_stairs, false);
        piece.generate_box(level, chunk_bb, 0, 4, 8, 6, 4, 8, south_stairs, south_stairs, false);
        piece.place_block(level, north_stairs.set(STAIRS_SHAPE, StairsShape::OuterRight), 0, 4, 1, chunk_bb);
        piece.place_block(level, north_stairs.set(STAIRS_SHAPE, StairsShape::OuterLeft), 6, 4, 1, chunk_bb);
        piece.place_block(level, south_stairs.set(STAIRS_SHAPE, StairsShape::OuterLeft), 0, 4, 8, chunk_bb);
        piece.place_block(level, south_stairs.set(STAIRS_SHAPE, StairsShape::OuterRight), 6, 4, 8, chunk_bb);
        for z in [2, 7] {
            for x in [1, 5] {
                piece.fill_column_down(level, log, x, -1, z, chunk_bb);
            }
        }
        if !self.spawned_witch && chunk_bb.is_inside(piece.get_world_pos(2, 2, 5)) {
            self.spawned_witch = true;
        }
        self.spawn_cat(chunk_bb);
    }

    fn spawn_cat(&mut self, chunk_bb: &BoundingBox) {
        if !self.spawned_cat && chunk_bb.is_inside(self.scattered.piece.get_world_pos(2, 2, 5)) {
            self.spawned_cat = true;
        }
    }
}
