use crisp_core::BlockPos;

use super::igloo_pieces;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, on_top_of_chunk_center};

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    on_top_of_chunk_center(
        context,
        HeightmapType::WorldSurfaceWg,
        Box::new(|builder, context| {
            let start_pos = BlockPos::new(context.chunk_pos.min_block_x(), 90, context.chunk_pos.min_block_z());
            let rotation = Rotation::get_random(&mut context.random);
            igloo_pieces::add_pieces(start_pos, rotation, builder, &mut context.random);
        }),
    )
}
