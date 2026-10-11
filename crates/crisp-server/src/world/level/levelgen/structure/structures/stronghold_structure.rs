use crisp_core::BlockPos;
use crisp_core::random::RandomSource;

use super::Piece;
use super::stronghold_pieces::{StartPiece, StrongholdPieceKind, StrongholdPieces};
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub};

pub fn find_generation_point(context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
    Some(GenerationStub {
        position: BlockPos::new(context.chunk_pos.min_block_x(), 0, context.chunk_pos.min_block_z()),
        generator: Box::new(generate_pieces),
    })
}

fn generate_pieces(builder: &mut StructurePiecesBuilder, context: &mut GenerationContext) {
    let mut tries: i64 = 0;
    let mut pieces = StrongholdPieces::default();
    loop {
        builder.clear();
        context.random.set_large_feature_seed(context.seed.wrapping_add(tries), context.chunk_pos.x, context.chunk_pos.z);
        tries += 1;
        pieces.reset_pieces();
        let mut start_room = StartPiece::new(&mut context.random, context.chunk_pos.min_block_x() + 2, context.chunk_pos.min_block_z() + 2);
        builder.add_piece(Piece::Stronghold(StrongholdPieceKind::Start(start_room.clone())));
        let start_stairs = start_room.stairs_down.clone();
        start_stairs.add_children(&mut pieces, &mut start_room, builder, &mut context.random);
        while !start_room.pending_children.is_empty() {
            let pos = context.random.next_int_bounded(start_room.pending_children.len() as i32) as usize;
            let structure_piece = start_room.pending_children.remove(pos);
            structure_piece.add_children(&mut pieces, &mut start_room, builder, &mut context.random);
        }
        builder.move_below_sea_level(context.chunk_generator.sea_level(), context.chunk_generator.min_y(), &mut context.random, 10);
        if !builder.is_empty() && start_room.portal_room_piece.is_some() {
            break;
        }
    }
}
