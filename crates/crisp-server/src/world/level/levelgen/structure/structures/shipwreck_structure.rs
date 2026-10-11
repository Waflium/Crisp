use crisp_core::BlockPos;

use super::Piece;
use super::shipwreck_pieces;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, get_lowest_y_at, get_mean_first_occupied_height, on_top_of_chunk_center};

pub fn find_generation_point(context: &mut GenerationContext, is_beached: bool) -> Option<GenerationStub<'static>> {
    let heightmap_type = if is_beached { HeightmapType::WorldSurfaceWg } else { HeightmapType::OceanFloorWg };
    on_top_of_chunk_center(context, heightmap_type, Box::new(move |builder, context| generate_pieces(builder, context, is_beached)))
}

fn generate_pieces(builder: &mut StructurePiecesBuilder, context: &mut GenerationContext, is_beached: bool) {
    let rotation = Rotation::get_random(&mut context.random);
    let offset = BlockPos::new(context.chunk_pos.min_block_x(), 90, context.chunk_pos.min_block_z());
    let mut piece = shipwreck_pieces::add_random_piece(offset, rotation, &mut context.random, is_beached);
    if piece.is_too_big_to_fit_in_world_gen_region() {
        let bb = piece.template.piece.get_bounding_box();
        let height = if is_beached {
            let min_y = get_lowest_y_at(context, bb.min.x, bb.x_span(), bb.min.z, bb.z_span());
            piece.calculate_beached_position(min_y, &mut context.random)
        } else {
            get_mean_first_occupied_height(context, bb.min.x, bb.x_span(), bb.min.z, bb.z_span())
        };
        piece.adjust_position_height(height);
    }
    builder.add_piece(Piece::Shipwreck(piece));
}
