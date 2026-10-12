use crisp_core::{BlockPos, Direction};
use crisp_protocol::nbt::Nbt;

use super::BoundingBox;
use super::structure_piece::{StructurePiece, make_bounding_box};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::chunk::HeightmapType;

#[derive(Clone)]
pub struct ScatteredFeaturePiece {
    pub piece: StructurePiece,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub height_position: i32,
}

fn int(tag: &Nbt, key: &str) -> i32 {
    match tag.get(key) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    }
}

impl ScatteredFeaturePiece {
    #[allow(clippy::too_many_arguments)]
    pub fn new(piece_type: &'static str, west: i32, floor: i32, north: i32, width: i32, height: i32, depth: i32, direction: Direction) -> ScatteredFeaturePiece {
        let mut piece = StructurePiece::new(piece_type, 0, make_bounding_box(west, floor, north, direction, width, height, depth));
        piece.set_orientation(Some(direction));
        ScatteredFeaturePiece { piece, width, height, depth, height_position: -1 }
    }

    pub fn load(piece_type: &'static str, tag: &Nbt) -> Option<ScatteredFeaturePiece> {
        Some(ScatteredFeaturePiece {
            piece: StructurePiece::load(piece_type, tag)?,
            width: int(tag, "Width"),
            height: int(tag, "Height"),
            depth: int(tag, "Depth"),
            height_position: int(tag, "HPos"),
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("Width".into(), Nbt::Int(self.width)));
        tag.push(("Height".into(), Nbt::Int(self.height)));
        tag.push(("Depth".into(), Nbt::Int(self.depth)));
        tag.push(("HPos".into(), Nbt::Int(self.height_position)));
    }

    pub fn update_average_ground_height(&mut self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, offset: i32) -> bool {
        if self.height_position >= 0 {
            return true;
        }
        let mut total = 0;
        let mut count = 0;
        let bounding_box = self.piece.bounding_box;
        for z in bounding_box.min.z..=bounding_box.max.z {
            for x in bounding_box.min.x..=bounding_box.max.x {
                if chunk_bb.is_inside(BlockPos::new(x, 64, z)) {
                    total += level.get_height(HeightmapType::MotionBlockingNoLeaves, x, z);
                    count += 1;
                }
            }
        }
        if count == 0 {
            return false;
        }
        self.height_position = total / count;
        let dy = self.height_position - self.piece.bounding_box.min.y + offset;
        self.piece.bounding_box.move_in_place(0, dy, 0);
        true
    }

    pub fn update_height_position_to_lowest_ground_height(&mut self, level: &mut impl WorldGenLevel, offset: i32) -> bool {
        if self.height_position >= 0 {
            return true;
        }
        let mut lowest_ground_height = level.get_max_y() + 1;
        let mut found_position_within_bounding_box = false;
        let bounding_box = self.piece.bounding_box;
        for z in bounding_box.min.z..=bounding_box.max.z {
            for x in bounding_box.min.x..=bounding_box.max.x {
                lowest_ground_height = lowest_ground_height.min(level.get_height(HeightmapType::MotionBlockingNoLeaves, x, z));
                found_position_within_bounding_box = true;
            }
        }
        if !found_position_within_bounding_box {
            return false;
        }
        self.height_position = lowest_ground_height;
        let dy = self.height_position - self.piece.bounding_box.min.y + offset;
        self.piece.bounding_box.move_in_place(0, dy, 0);
        true
    }
}
