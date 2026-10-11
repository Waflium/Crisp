use super::pieces_container::PiecesContainer;
use crate::world::level::levelgen::structure::structures::Piece;

#[derive(Default)]
pub struct StructurePiecesBuilder {
    pieces: Vec<Piece>,
}

impl StructurePiecesBuilder {
    pub fn add_piece(&mut self, piece: Piece) {
        self.pieces.push(piece);
    }

    pub fn build(self) -> PiecesContainer {
        PiecesContainer { pieces: self.pieces }
    }
}
