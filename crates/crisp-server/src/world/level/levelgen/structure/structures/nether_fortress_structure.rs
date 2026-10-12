use crisp_core::BlockPos;
use crisp_core::random::RandomSource;

use super::Piece;
use super::nether_fortress_pieces::{NetherBridgePieceKind, StartPiece};
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub};

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    Some(GenerationStub {
        position: BlockPos::new(context.chunk_pos.min_block_x(), 64, context.chunk_pos.min_block_z()),
        generator: Box::new(generate_pieces),
    })
}

fn generate_pieces(builder: &mut StructurePiecesBuilder, context: &mut GenerationContext) {
    let mut start = StartPiece::new(&mut context.random, context.chunk_pos.min_block_x() + 2, context.chunk_pos.min_block_z() + 2);
    builder.add_piece(Piece::NetherFortress(NetherBridgePieceKind::BridgeCrossing(start.bridge_crossing.clone())));
    start.bridge_crossing.clone().add_children(&mut start, builder, &mut context.random);
    while !start.pending_children.is_empty() {
        let pos = context.random.next_int_bounded(start.pending_children.len() as i32) as usize;
        let structure_piece = start.pending_children.remove(pos);
        structure_piece.add_children(&mut start, builder, &mut context.random);
    }
    builder.move_inside_heights(&mut context.random, 48, 70);
}
