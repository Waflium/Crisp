use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, ChunkPos};
use crisp_protocol::nbt::Nbt;

use super::BoundingBox;
use super::pieces::pieces_container::PiecesContainer;
use super::structure::StructureHolder;
use super::structures::{StructureType, ocean_monument_structure};
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct StructureStart {
    pub structure: StructureHolder,
    pub chunk_pos: ChunkPos,
    references: AtomicI32,
    pub piece_container: Mutex<PiecesContainer>,
    cached_bounding_box: OnceLock<BoundingBox>,
}

fn int(tag: &Nbt, key: &str) -> i32 {
    match tag.get(key) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    }
}

impl StructureStart {
    pub fn new(structure: StructureHolder, chunk_pos: ChunkPos, references: i32, piece_container: PiecesContainer) -> StructureStart {
        StructureStart { structure, chunk_pos, references: AtomicI32::new(references), piece_container: Mutex::new(piece_container), cached_bounding_box: OnceLock::new() }
    }

    pub fn load_static_start(tag: &Nbt, seed: i64) -> Option<Option<StructureStart>> {
        let id = tag.get("id").and_then(Nbt::as_str).unwrap_or("");
        if id == "INVALID" {
            return Some(None);
        }
        let Some(structure) = StructureHolder::by_name(id) else {
            error!("Unknown stucture id: {id}");
            return None;
        };
        let chunk_pos = ChunkPos::new(int(tag, "ChunkX"), int(tag, "ChunkZ"));
        let references = int(tag, "references");
        let children = tag.get("Children").map(Nbt::as_list).unwrap_or(&[]);
        let mut pieces = PiecesContainer::load(children);
        if let StructureType::OceanMonument = structure.value().structure_type {
            pieces = ocean_monument_structure::regenerate_pieces_after_load(chunk_pos, seed, pieces);
        }
        Some(Some(StructureStart::new(structure, chunk_pos, references, pieces)))
    }

    pub fn get_bounding_box(&self) -> BoundingBox {
        *self.cached_bounding_box.get_or_init(|| self.structure.value().adjust_bounding_box(self.piece_container.lock().unwrap().calculate_bounding_box()))
    }

    pub fn place_in_chunk(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, chunk_pos: ChunkPos) {
        let mut container = self.piece_container.lock().unwrap();
        let Some(first) = container.pieces.first() else { return };
        let center_bb = first.piece().bounding_box;
        let center_pos = center_bb.get_center();
        let reference_pos = BlockPos::new(center_pos.x, center_bb.min.y, center_pos.z);
        for next in container.pieces.iter_mut() {
            if next.piece().bounding_box.intersects(chunk_bb) {
                next.post_process(level, random, chunk_bb, chunk_pos, reference_pos);
            }
        }
        self.structure.value().structure_type.after_place(level, random, chunk_bb, chunk_pos, &container);
    }

    pub fn create_tag(&self, chunk_pos: ChunkPos) -> Nbt {
        Nbt::Compound(vec![
            ("id".into(), Nbt::String(self.structure.key().to_owned())),
            ("ChunkX".into(), Nbt::Int(chunk_pos.x)),
            ("ChunkZ".into(), Nbt::Int(chunk_pos.z)),
            ("references".into(), Nbt::Int(self.get_references())),
            ("Children".into(), self.piece_container.lock().unwrap().save()),
        ])
    }

    pub fn is_valid(&self) -> bool {
        !self.piece_container.lock().unwrap().is_empty()
    }

    pub fn can_be_referenced(&self) -> bool {
        self.get_references() < self.get_max_references()
    }

    pub fn add_reference(&self) {
        self.references.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_references(&self) -> i32 {
        self.references.load(Ordering::Relaxed)
    }

    fn get_max_references(&self) -> i32 {
        1
    }
}
