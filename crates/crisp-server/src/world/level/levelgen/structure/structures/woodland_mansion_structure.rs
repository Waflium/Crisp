use crisp_core::BlockPos;
use crisp_data::blocks;

use super::Piece;
use super::woodland_mansion_pieces;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::pieces_container::PiecesContainer;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, get_lowest_y_in_5_by_5_box};

const START_OFFSET_IN_CHUNK: i32 = 7;

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    let chunk_pos = context.chunk_pos;
    let block_x = chunk_pos.min_block_x() + START_OFFSET_IN_CHUNK;
    let block_z = chunk_pos.min_block_z() + START_OFFSET_IN_CHUNK;
    if !context.could_valid_biome_exist_in_terrain_column(block_x, block_z) {
        return None;
    }
    let rotation = Rotation::get_random(&mut context.random);
    let start_pos = get_lowest_y_in_5_by_5_box(context, block_x, block_z, rotation);
    if start_pos.y < 60 {
        return None;
    }
    Some(GenerationStub { position: start_pos, generator: Box::new(move |builder, context| generate_pieces(builder, context, start_pos, rotation)) })
}

fn generate_pieces(builder: &mut StructurePiecesBuilder, context: &mut GenerationContext, start_pos: BlockPos, rotation: Rotation) {
    let mut wm_pieces = Vec::new();
    woodland_mansion_pieces::generate_mansion(start_pos, rotation, &mut wm_pieces, &mut context.random);
    for piece in wm_pieces {
        builder.add_piece(Piece::WoodlandMansion(piece));
    }
}

pub fn after_place(level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, pieces: &PiecesContainer) {
    let min_y = level.get_min_y();
    let bounding_box = pieces.calculate_bounding_box();
    let y_start = bounding_box.min.y;
    for x in chunk_bb.min.x..=chunk_bb.max.x {
        for z in chunk_bb.min.z..=chunk_bb.max.z {
            let mut pos = BlockPos::new(x, y_start, z);
            if !level.get_block_state(pos).is_air() && bounding_box.is_inside(pos) && pieces.is_inside_piece(pos) {
                for y in (min_y + 1..y_start).rev() {
                    pos.y = y;
                    let state = level.get_block_state(pos);
                    if !state.is_air() && !state.liquid() {
                        break;
                    }
                    level.set_block(pos, blocks::COBBLESTONE.default_state(), 2);
                }
            }
        }
    }
}
