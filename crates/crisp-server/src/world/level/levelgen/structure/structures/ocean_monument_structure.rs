use crisp_core::random::generate_unique_seed;
use crisp_core::{ChunkPos, Direction};

use super::Piece;
use super::ocean_monument_pieces::MonumentBuilding;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::quart_pos;
use crate::world::level::levelgen::structure::pieces::pieces_container::PiecesContainer;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, on_top_of_chunk_center};
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;

const BIOME_RANGE_CHECK: i32 = 29;

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    let offset_x = context.chunk_pos.min_block_x() + 9;
    let offset_z = context.chunk_pos.min_block_z() + 9;
    let required_surrounding = crisp_data::tag("minecraft:worldgen/biome", "minecraft:required_ocean_monument_surrounding");
    let sea_level = context.chunk_generator.sea_level();
    let (x0, y0, z0) = (quart_pos::from_block(offset_x - BIOME_RANGE_CHECK), quart_pos::from_block(sea_level - BIOME_RANGE_CHECK), quart_pos::from_block(offset_z - BIOME_RANGE_CHECK));
    let (x1, y1, z1) = (quart_pos::from_block(offset_x + BIOME_RANGE_CHECK), quart_pos::from_block(sea_level + BIOME_RANGE_CHECK), quart_pos::from_block(offset_z + BIOME_RANGE_CHECK));
    for noise_z in z0..=z1 {
        for noise_x in x0..=x1 {
            for noise_y in y0..=y1 {
                if !required_surrounding.contains(&(context.get_noise_biome(noise_x, noise_y, noise_z).0 as i32)) {
                    return None;
                }
            }
        }
    }
    on_top_of_chunk_center(context, HeightmapType::OceanFloorWg, Box::new(generate_pieces))
}

fn create_top_piece(chunk_pos: ChunkPos, random: &mut WorldgenRandom) -> Piece {
    let west = chunk_pos.min_block_x() - 29;
    let north = chunk_pos.min_block_z() - 29;
    let orientation = Direction::get_random_horizontal_direction(random);
    Piece::OceanMonumentBuilding(MonumentBuilding::new(random, west, north, orientation))
}

fn generate_pieces(builder: &mut StructurePiecesBuilder, context: &mut GenerationContext) {
    builder.add_piece(create_top_piece(context.chunk_pos, &mut context.random));
}

pub fn regenerate_pieces_after_load(chunk_pos: ChunkPos, seed: i64, saved_pieces: PiecesContainer) -> PiecesContainer {
    if saved_pieces.is_empty() {
        return saved_pieces;
    }
    let mut random = WorldgenRandom::legacy(generate_unique_seed());
    random.set_large_feature_seed(seed, chunk_pos.x, chunk_pos.z);
    let old_top_piece = saved_pieces.pieces[0].piece();
    let old_bounding_box = old_top_piece.get_bounding_box();
    let west = old_bounding_box.min.x;
    let north = old_bounding_box.min.z;
    let default_orientation = Direction::get_random_horizontal_direction(&mut random);
    let orientation = old_top_piece.get_orientation().unwrap_or(default_orientation);
    let top_piece = MonumentBuilding::new(&mut random, west, north, orientation);
    let mut result = StructurePiecesBuilder::default();
    result.add_piece(Piece::OceanMonumentBuilding(top_piece));
    result.build()
}
