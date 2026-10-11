use crisp_core::BlockPos;

use super::Piece;
use super::buried_treasure_pieces::BuriedTreasurePiece;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, on_top_of_chunk_center};

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    on_top_of_chunk_center(
        context,
        HeightmapType::OceanFloorWg,
        Box::new(|builder, context| {
            let offset = BlockPos::new(context.chunk_pos.min_block_x() + 9, 90, context.chunk_pos.min_block_z() + 9);
            builder.add_piece(Piece::BuriedTreasure(BuriedTreasurePiece::new(offset)));
        }),
    )
}
