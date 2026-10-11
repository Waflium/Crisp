use super::structure::{GenerationContext, GenerationStub, get_lowest_y, on_top_of_chunk_center_without_biome_check};
use super::structures::Piece;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;

pub type PieceConstructor = fn(&mut WorldgenRandom, i32, i32) -> Piece;

pub fn find_generation_point(context: &mut GenerationContext, constructor: PieceConstructor, width: i32, depth: i32) -> Option<GenerationStub<'static>> {
    if !context.could_valid_biome_exist_on_top_of_chunk_center() {
        return None;
    }
    if get_lowest_y(context, width, depth) < context.chunk_generator.sea_level() {
        return None;
    }
    on_top_of_chunk_center_without_biome_check(
        context,
        HeightmapType::WorldSurfaceWg,
        Box::new(move |builder, context| {
            let chunk_pos = context.chunk_pos;
            builder.add_piece(constructor(&mut context.random, chunk_pos.min_block_x(), chunk_pos.min_block_z()));
        }),
    )
}
