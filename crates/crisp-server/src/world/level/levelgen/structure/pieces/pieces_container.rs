use crisp_protocol::nbt::Nbt;

use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structures::Piece;

pub struct PiecesContainer {
    pub pieces: Vec<Piece>,
}

impl PiecesContainer {
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    pub fn save(&self) -> Nbt {
        Nbt::List(self.pieces.iter().map(Piece::create_tag).collect())
    }

    pub fn load(children: &[Nbt]) -> PiecesContainer {
        let mut pieces = Vec::with_capacity(children.len());
        for piece_tag in children {
            let old_id = piece_tag.get("id").and_then(Nbt::as_str).unwrap_or("").to_lowercase();
            let piece_id = if old_id.contains(':') { old_id } else { format!("minecraft:{old_id}") };
            match Piece::load(&piece_id, piece_tag) {
                Some(piece) => pieces.push(piece),
                None => error!("Unknown structure piece id: {piece_id}"),
            }
        }
        PiecesContainer { pieces }
    }

    pub fn is_inside_piece(&self, start_pos: crisp_core::BlockPos) -> bool {
        self.pieces.iter().any(|piece| piece.piece().get_bounding_box().is_inside(start_pos))
    }

    pub fn calculate_bounding_box(&self) -> BoundingBox {
        BoundingBox::encapsulating_boxes(self.pieces.iter().map(|piece| &piece.piece().bounding_box)).expect("Unable to calculate boundingbox without pieces")
    }
}
