use super::Piece;
use super::swamp_hut_piece::SwampHutPiece;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, on_top_of_chunk_center};

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    on_top_of_chunk_center(
        context,
        HeightmapType::WorldSurfaceWg,
        Box::new(|builder, context| {
            let (x, z) = (context.chunk_pos.min_block_x(), context.chunk_pos.min_block_z());
            builder.add_piece(Piece::SwampHut(SwampHutPiece::new(&mut context.random, x, z)));
        }),
    )
}
