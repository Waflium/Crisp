use std::sync::Arc;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use super::BoundingBox;
use super::pools::jigsaw_junction::JigsawJunction;
use super::pools::structure_pool_element::{StructurePoolElement, liquid_settings_name};
use super::structure_piece::StructurePiece;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block::rotation::Rotation;

pub const JIGSAW: &str = "minecraft:jigsaw";

#[derive(Clone)]
pub struct PoolElementStructurePiece {
    pub piece: StructurePiece,
    pub element: Arc<StructurePoolElement>,
    pub position: BlockPos,
    ground_level_delta: i32,
    pub rotation: Rotation,
    pub junctions: Vec<JigsawJunction>,
    apply_waterlogging: bool,
}

fn int(tag: &Nbt, key: &str) -> i32 {
    match tag.get(key) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    }
}

impl PoolElementStructurePiece {
    pub fn new(element: Arc<StructurePoolElement>, position: BlockPos, ground_level_delta: i32, rotation: Rotation, bounding_box: BoundingBox, apply_waterlogging: bool) -> PoolElementStructurePiece {
        PoolElementStructurePiece { piece: StructurePiece::new(JIGSAW, 0, bounding_box), element, position, ground_level_delta, rotation, junctions: Vec::new(), apply_waterlogging }
    }

    pub fn load(tag: &Nbt) -> Option<PoolElementStructurePiece> {
        let mut piece = StructurePiece::load(JIGSAW, tag)?;
        let position = BlockPos::new(int(tag, "PosX"), int(tag, "PosY"), int(tag, "PosZ"));
        let element = Arc::new(StructurePoolElement::load(tag.get("pool_element")?).ok()?);
        let rotation = tag.get("rotation").and_then(Nbt::as_str).and_then(Rotation::by_legacy_name)?;
        piece.bounding_box = element.get_bounding_box(position, rotation);
        let junctions = tag.get("junctions").map(Nbt::as_list).unwrap_or(&[]).iter().map(JigsawJunction::deserialize).collect();
        let apply_waterlogging = tag.get("liquid_settings").and_then(Nbt::as_str) != Some("ignore_waterlogging");
        Some(PoolElementStructurePiece { piece, element, position, ground_level_delta: int(tag, "ground_level_delta"), rotation, junctions, apply_waterlogging })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("PosX".into(), Nbt::Int(self.position.x)));
        tag.push(("PosY".into(), Nbt::Int(self.position.y)));
        tag.push(("PosZ".into(), Nbt::Int(self.position.z)));
        tag.push(("ground_level_delta".into(), Nbt::Int(self.ground_level_delta)));
        tag.push(("pool_element".into(), self.element.to_nbt()));
        tag.push(("rotation".into(), Nbt::String(self.rotation.legacy_name().to_owned())));
        tag.push(("junctions".into(), Nbt::List(self.junctions.iter().map(JigsawJunction::serialize).collect())));
        if !self.apply_waterlogging {
            tag.push(("liquid_settings".into(), Nbt::String(liquid_settings_name(self.apply_waterlogging).to_owned())));
        }
    }

    pub fn post_process(&mut self, level: &mut WorldGenRegion, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        self.place(level, random, chunk_bb, reference_pos, false);
    }

    pub fn place(&mut self, level: &mut WorldGenRegion, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos, keep_jigsaws: bool) {
        self.element.place(level, self.position, reference_pos, self.rotation, chunk_bb, random, self.apply_waterlogging, keep_jigsaws);
    }

    pub fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.piece.move_by(dx, dy, dz);
        self.position = self.position.offset(dx, dy, dz);
    }

    pub fn get_ground_level_delta(&self) -> i32 {
        self.ground_level_delta
    }
}
