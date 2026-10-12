use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structure_piece::StructurePiece;

pub const BURIED_TREASURE_PIECE: &str = "minecraft:btp";
const BURIED_TREASURE: &str = "minecraft:chests/buried_treasure";

#[derive(Clone)]
pub struct BuriedTreasurePiece {
    pub piece: StructurePiece,
}

fn is_liquid(block_state: BlockState) -> bool {
    block_state.is_block(blocks::WATER) || block_state.is_block(blocks::LAVA)
}

impl BuriedTreasurePiece {
    pub fn new(offset: BlockPos) -> BuriedTreasurePiece {
        BuriedTreasurePiece { piece: StructurePiece::new(BURIED_TREASURE_PIECE, 0, BoundingBox::of(offset)) }
    }

    pub fn load(tag: &Nbt) -> Option<BuriedTreasurePiece> {
        Some(BuriedTreasurePiece { piece: StructurePiece::load(BURIED_TREASURE_PIECE, tag)? })
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let bounding_box = self.piece.bounding_box;
        let y = level.get_height(HeightmapType::OceanFloorWg, bounding_box.min.x, bounding_box.min.z);
        let mut pos = BlockPos::new(bounding_box.min.x, y, bounding_box.min.z);
        while pos.y > level.get_min_y() {
            let current_state = level.get_block_state(pos);
            let below_state = level.get_block_state(pos.below());
            if below_state.is_block(blocks::SANDSTONE)
                || below_state.is_block(blocks::STONE)
                || below_state.is_block(blocks::ANDESITE)
                || below_state.is_block(blocks::GRANITE)
                || below_state.is_block(blocks::DIORITE)
            {
                let soft_state = if !current_state.is_air() && !is_liquid(current_state) { current_state } else { blocks::SAND.default_state() };
                for direction in Direction::VALUES {
                    let relative_pos = pos.relative(direction);
                    let relative_state = level.get_block_state(relative_pos);
                    if relative_state.is_air() || is_liquid(relative_state) {
                        let below_relative_state = level.get_block_state(relative_pos.below());
                        if (below_relative_state.is_air() || is_liquid(below_relative_state)) && direction != Direction::Up {
                            level.set_block_and_update(relative_pos, below_state);
                        } else {
                            level.set_block_and_update(relative_pos, soft_state);
                        }
                    }
                }
                self.piece.bounding_box = BoundingBox::of(pos);
                self.piece.create_chest(level, chunk_bb, random, pos, BURIED_TREASURE, None);
                return;
            }
            pos = pos.below();
        }
    }
}
