use crisp_core::BlockPos;
use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_data::blocks;

use super::Piece;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::pieces_container::PiecesContainer;

const DESERT_PYRAMID_ARCHAEOLOGY: &str = "minecraft:archaeology/desert_pyramid";

pub fn after_place(level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, pieces: &PiecesContainer) {
    let mut unique_sand_placements = Vec::new();
    for piece in &pieces.pieces {
        if let Piece::DesertPyramid(desert_pyramid_piece) = piece {
            unique_sand_placements.extend_from_slice(desert_pyramid_piece.get_potential_suspicious_sand_world_positions());
            place_suspicious_sand(chunk_bb, level, desert_pyramid_piece.get_random_collapsed_roof_pos());
        }
    }
    unique_sand_placements.sort_unstable_by_key(|pos: &BlockPos| (pos.y, pos.z, pos.x));
    unique_sand_placements.dedup();
    let mut shuffled_sand_placements = unique_sand_placements;
    let center = pieces.calculate_bounding_box().get_center();
    let mut positional_random = LegacyRandomSource::new(level.get_seed()).fork_positional().at(center.x, center.y, center.z);
    util::shuffle(&mut shuffled_sand_placements, &mut positional_random);
    let mut suspicious_sand_to_place = (shuffled_sand_placements.len() as i32).min(positional_random.next_int_range(5, 8));
    for block_pos in shuffled_sand_placements {
        if suspicious_sand_to_place > 0 {
            suspicious_sand_to_place -= 1;
            place_suspicious_sand(chunk_bb, level, block_pos);
        } else if chunk_bb.is_inside(block_pos) {
            level.set_block(block_pos, blocks::SAND.default_state(), 2);
        }
    }
}

fn place_suspicious_sand(chunk_bb: &BoundingBox, level: &mut impl WorldGenLevel, block_pos: BlockPos) {
    if chunk_bb.is_inside(block_pos) {
        level.set_block(block_pos, blocks::SUSPICIOUS_SAND.default_state(), 2);
        if let Some(entity) = level.get_block_entity(block_pos)
            && let BlockEntityKind::Brushable(brushable) = &mut entity.kind
        {
            brushable.set_loot_table(DESERT_PYRAMID_ARCHAEOLOGY, block_pos.as_long());
        }
    }
}
