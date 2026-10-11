use crisp_core::random::RandomSource;

use super::pieces_container::PiecesContainer;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structure_piece::find_collision_piece;
use crate::world::level::levelgen::structure::structures::Piece;

#[derive(Default)]
pub struct StructurePiecesBuilder {
    pieces: Vec<Piece>,
}

impl StructurePiecesBuilder {
    pub fn add_piece(&mut self, piece: Piece) {
        self.pieces.push(piece);
    }

    pub fn find_collision_piece(&self, bounding_box: &BoundingBox) -> Option<&Piece> {
        find_collision_piece(&self.pieces, bounding_box)
    }

    pub fn offset_pieces_vertically(&mut self, dy: i32) {
        for piece in &mut self.pieces {
            piece.piece_mut().move_by(0, dy, 0);
        }
    }

    pub fn move_below_sea_level(&mut self, sea_level: i32, min_y: i32, random: &mut impl RandomSource, offset: i32) -> i32 {
        let max_y = sea_level - offset;
        let bounding_box = self.get_bounding_box();
        let mut y1_pos = bounding_box.y_span() + min_y + 1;
        if y1_pos < max_y {
            y1_pos += random.next_int_bounded(max_y - y1_pos);
        }
        let dy = y1_pos - bounding_box.max.y;
        self.offset_pieces_vertically(dy);
        dy
    }

    pub fn build(self) -> PiecesContainer {
        PiecesContainer { pieces: self.pieces }
    }

    pub fn clear(&mut self) {
        self.pieces.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    pub fn get_bounding_box(&self) -> BoundingBox {
        BoundingBox::encapsulating_boxes(self.pieces.iter().map(|piece| &piece.piece().bounding_box)).expect("Unable to calculate boundingbox without pieces")
    }
}
