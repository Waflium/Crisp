use crisp_core::ChunkPos;
use crisp_core::hash_map::{FastMap, FastSet};
use crisp_protocol::nbt::Nbt;
use crisp_data::DimensionType;

use super::placement::structure_placement::StructurePlacement;
use super::structure::{GenerationContext, StructureHolder};
use super::structure_check_result::StructureCheckResult;
use crate::server::level::chunk_workers::IoWorker;
use crate::world::level::chunk::ChunkGenerator;

const MAX_CHUNKS_WITHOUT_STORED_STARTS: usize = 131072;

pub struct StructureCheck {
    storage_access: Option<IoWorker>,
    chunk_generator: ChunkGenerator,
    height_accessor: &'static DimensionType,
    seed: i64,
    loaded_chunks: FastMap<i64, FastMap<StructureHolder, i32>>,
    feature_checks: FastMap<StructureHolder, FastMap<i64, bool>>,
    chunks_without_starts_in_storage: FastSet<i64>,
}

impl StructureCheck {
    pub fn new(storage_access: Option<IoWorker>, chunk_generator: ChunkGenerator, height_accessor: &'static DimensionType, seed: i64) -> StructureCheck {
        StructureCheck {
            storage_access,
            chunk_generator,
            height_accessor,
            seed,
            loaded_chunks: FastMap::default(),
            feature_checks: FastMap::default(),
            chunks_without_starts_in_storage: FastSet::default(),
        }
    }

    pub fn check_start(&mut self, pos: ChunkPos, structure: StructureHolder, placement: &StructurePlacement, require_unreferenced: bool) -> StructureCheckResult {
        let pos_key = pos.pack();
        if let Some(cached_result) = self.loaded_chunks.get(&pos_key) {
            return Self::check_structure_info(cached_result, structure, require_unreferenced);
        }
        if !self.chunks_without_starts_in_storage.contains(&pos_key) {
            if let Some(storage_check_result) = self.try_load_from_storage(pos, structure, require_unreferenced) {
                return storage_check_result;
            }
            if self.chunks_without_starts_in_storage.len() >= MAX_CHUNKS_WITHOUT_STORED_STARTS {
                self.chunks_without_starts_in_storage.clear();
            }
            self.chunks_without_starts_in_storage.insert(pos_key);
        }
        if !placement.apply_additional_chunk_restrictions(pos.x, pos.z, self.seed) {
            return StructureCheckResult::StartNotPresent;
        }
        let checks = self.feature_checks.entry(structure).or_default();
        let is_feature_chunk = match checks.get(&pos_key) {
            Some(&result) => result,
            None => {
                let result = Self::can_create_structure(&self.chunk_generator, self.height_accessor, self.seed, pos, structure);
                checks.insert(pos_key, result);
                result
            }
        };
        if is_feature_chunk { StructureCheckResult::ChunkLoadNeeded } else { StructureCheckResult::StartNotPresent }
    }

    fn can_create_structure(chunk_generator: &ChunkGenerator, height_accessor: &DimensionType, seed: i64, pos: ChunkPos, structure: StructureHolder) -> bool {
        let structure = structure.value();
        let mut context = GenerationContext::new(chunk_generator, seed, pos, height_accessor.min_y, height_accessor.height as i32, structure.biomes());
        structure.find_valid_generation_point(&mut context).is_some()
    }

    fn try_load_from_storage(&mut self, pos: ChunkPos, structure: StructureHolder, require_unreferenced: bool) -> Option<StructureCheckResult> {
        let storage_access = self.storage_access.as_ref()?;
        let chunk_tag = match storage_access.scan_chunk(pos) {
            Ok(chunk_tag) => chunk_tag?,
            Err(e) => {
                warn!("Failed to read chunk {pos:?}: {e}");
                return Some(StructureCheckResult::ChunkLoadNeeded);
            }
        };
        let known_starts = Self::load_structures(&chunk_tag)?;
        let result = Self::check_structure_info(&known_starts, structure, require_unreferenced);
        self.store_full_results(pos.pack(), known_starts);
        Some(result)
    }

    fn load_structures(chunk_tag: &Nbt) -> Option<FastMap<StructureHolder, i32>> {
        let Some(Nbt::Compound(starts_tag)) = chunk_tag.get("structures").filter(|tag| matches!(tag, Nbt::Compound(_))).and_then(|tag| tag.get("starts")) else {
            return None;
        };
        let mut known_starts = FastMap::default();
        for (key, tag) in starts_tag {
            let Some(found_feature) = StructureHolder::by_name(key) else { continue };
            if !matches!(tag, Nbt::Compound(_)) {
                continue;
            }
            let piece_id = tag.get("id").and_then(Nbt::as_str).unwrap_or("");
            if piece_id != "INVALID" {
                let reference_count = tag.get("references").and_then(Nbt::as_i64).unwrap_or(0) as i32;
                known_starts.insert(found_feature, reference_count);
            }
        }
        Some(known_starts)
    }

    fn check_structure_info(cached_result: &FastMap<StructureHolder, i32>, structure: StructureHolder, require_unreferenced: bool) -> StructureCheckResult {
        let reference_count = cached_result.get(&structure).copied().unwrap_or(-1);
        if reference_count == -1 || require_unreferenced && reference_count != 0 {
            StructureCheckResult::StartNotPresent
        } else {
            StructureCheckResult::StartPresent
        }
    }

    pub fn on_structure_load(&mut self, pos: ChunkPos, starts: FastMap<StructureHolder, i32>) {
        self.store_full_results(pos.pack(), starts);
    }

    fn store_full_results(&mut self, pos_key: i64, starts: FastMap<StructureHolder, i32>) {
        self.loaded_chunks.insert(pos_key, starts);
        for checks in self.feature_checks.values_mut() {
            checks.remove(&pos_key);
        }
        self.chunks_without_starts_in_storage.remove(&pos_key);
    }

    pub fn increment_reference(&mut self, chunk_pos: ChunkPos, structure: StructureHolder) {
        let pos_key = chunk_pos.pack();
        *self.loaded_chunks.entry(pos_key).or_default().entry(structure).or_insert(0) += 1;
        self.chunks_without_starts_in_storage.remove(&pos_key);
    }
}
