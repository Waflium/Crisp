use crisp_core::BlockPos;

use super::Piece;
use super::end_city_pieces;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, get_lowest_y_in_5_by_5_box};

const START_OFFSET_IN_CHUNK: i32 = 7;

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    let block_x = context.chunk_pos.min_block_x() + START_OFFSET_IN_CHUNK;
    let block_z = context.chunk_pos.min_block_z() + START_OFFSET_IN_CHUNK;
    if !context.could_valid_biome_exist_in_terrain_column(block_x, block_z) {
        return None;
    }
    let rotation = Rotation::get_random(&mut context.random);
    let start_pos = get_lowest_y_in_5_by_5_box(context, block_x, block_z, rotation);
    if start_pos.y < 60 {
        return None;
    }
    Some(GenerationStub { position: start_pos, generator: Box::new(move |builder, context| generate_pieces(builder, start_pos, rotation, context)) })
}

fn generate_pieces(builder: &mut StructurePiecesBuilder, start_pos: BlockPos, rotation: Rotation, context: &mut GenerationContext) {
    let pieces = end_city_pieces::start_house_tower(start_pos, rotation, &mut context.random);
    for piece in pieces {
        builder.add_piece(Piece::EndCity(piece));
    }
}
