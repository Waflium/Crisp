use crisp_core::{BlockPos, ChunkPos};

use super::poi_record::PoiRecord;
use super::poi_section::PoiSection;
use super::poi_types::{self, PoiType};
use crate::server::level::chunk_cache::ChunkCache;
use crate::server::level::chunk_workers::IoWorker;
use crate::server::level::chunk_level::ChunkStatus;
use crate::world::level::chunk::LevelChunkSection;
use crate::world::level::chunk::storage::section_storage::{SectionData, SectionStorage, section_key};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Occupancy {
    HasSpace,
    IsOccupied,
    Any,
}

impl Occupancy {
    pub fn test(self, record: &PoiRecord) -> bool {
        match self {
            Occupancy::HasSpace => record.has_space(),
            Occupancy::IsOccupied => record.is_occupied(),
            Occupancy::Any => true,
        }
    }
}

impl SectionData for PoiSection {
    fn pack(&self) -> crisp_protocol::nbt::Nbt {
        PoiSection::pack(self)
    }

    fn unpack(nbt: &crisp_protocol::nbt::Nbt) -> Option<PoiSection> {
        PoiSection::unpack(nbt)
    }
}

fn block_section_key(pos: BlockPos) -> i64 {
    section_key(pos.x >> 4, pos.y >> 4, pos.z >> 4)
}

pub struct PoiManager {
    pub storage: SectionStorage<PoiSection>,
    pub loaded_chunks: crisp_core::fastutil::LongOpenHashSet,
}

impl PoiManager {
    pub fn new(io: Option<IoWorker>, min_section_y: i32, max_section_y: i32) -> PoiManager {
        PoiManager { storage: SectionStorage::new(io, min_section_y, max_section_y), loaded_chunks: crisp_core::fastutil::LongOpenHashSet::new() }
    }

    pub fn add(&mut self, pos: BlockPos, poi_type: PoiType) -> Option<PoiRecord> {
        let key = block_section_key(pos);
        let record = self.storage.get_or_create(key).add(pos, poi_type);
        if record.is_some() {
            self.storage.set_dirty(key);
        }
        record
    }

    pub fn remove(&mut self, pos: BlockPos) {
        let key = block_section_key(pos);
        if let Some(section) = self.storage.get_or_load(key)
            && section.remove(pos)
        {
            self.storage.set_dirty(key);
        }
    }

    pub fn get_in_chunk(&mut self, predicate: &dyn Fn(PoiType) -> bool, chunk_pos: ChunkPos, occupancy: Occupancy, out: &mut Vec<PoiRecord>) {
        let (min_section_y, max_section_y) = self.section_range();
        for section_y in min_section_y..=max_section_y {
            if let Some(section) = self.storage.get_or_load(section_key(chunk_pos.x, section_y, chunk_pos.z)) {
                out.extend(section.get_records(predicate, occupancy).copied());
            }
        }
    }

    pub fn get_in_square(&mut self, predicate: &dyn Fn(PoiType) -> bool, center: BlockPos, radius: i32, occupancy: Occupancy) -> Vec<PoiRecord> {
        let chunk_radius = radius.div_euclid(16) + 1;
        let center_chunk = center.chunk();
        let mut records = Vec::new();
        for z in center_chunk.z - chunk_radius..=center_chunk.z + chunk_radius {
            for x in center_chunk.x - chunk_radius..=center_chunk.x + chunk_radius {
                self.get_in_chunk(predicate, ChunkPos::new(x, z), occupancy, &mut records);
            }
        }
        records.retain(|record| (record.pos.x - center.x).abs() <= radius && (record.pos.z - center.z).abs() <= radius);
        records
    }

    pub fn get_in_range(&mut self, predicate: &dyn Fn(PoiType) -> bool, center: BlockPos, radius: i32, occupancy: Occupancy) -> Vec<PoiRecord> {
        let radius_sqr = (radius * radius) as f64;
        let mut records = self.get_in_square(predicate, center, radius, occupancy);
        records.retain(|record| record.pos.dist_sqr(center) <= radius_sqr);
        records
    }

    pub fn find_closest(
        &mut self,
        predicate: &dyn Fn(PoiType) -> bool,
        filter: impl Fn(BlockPos) -> bool,
        center: BlockPos,
        radius: i32,
        occupancy: Occupancy,
    ) -> Option<BlockPos> {
        let mut closest: Option<BlockPos> = None;
        for pos in self.get_in_range(predicate, center, radius, occupancy).into_iter().map(|record| record.pos).filter(|&pos| filter(pos)) {
            if closest.is_none_or(|best| pos.dist_sqr(center) < best.dist_sqr(center)) {
                closest = Some(pos);
            }
        }
        closest
    }

    pub fn find_closest_with_type(&mut self, predicate: &dyn Fn(PoiType) -> bool, center: BlockPos, radius: i32, occupancy: Occupancy) -> Option<(PoiType, BlockPos)> {
        let mut closest: Option<PoiRecord> = None;
        for record in self.get_in_range(predicate, center, radius, occupancy) {
            if closest.is_none_or(|best| record.pos.dist_sqr(center) < best.pos.dist_sqr(center)) {
                closest = Some(record);
            }
        }
        closest.map(|record| (record.poi_type, record.pos))
    }

    pub fn exists(&mut self, pos: BlockPos, predicate: impl Fn(PoiType) -> bool) -> bool {
        self.storage.get_or_load(block_section_key(pos)).is_some_and(|section| section.exists(pos, predicate))
    }

    pub fn get_type(&mut self, pos: BlockPos) -> Option<PoiType> {
        self.storage.get_or_load(block_section_key(pos)).and_then(|section| section.get_type(pos))
    }

    pub fn release(&mut self, pos: BlockPos) -> bool {
        let key = block_section_key(pos);
        let section = self.storage.get_or_load(key).unwrap_or_else(|| panic!("POI never registered at {pos:?}"));
        let record = section.record_mut(pos).unwrap_or_else(|| panic!("POI never registered at {pos:?}"));
        let success = record.release_ticket();
        self.storage.set_dirty(key);
        success
    }

    pub fn tick(&mut self, have_time: &mut dyn FnMut() -> bool) {
        self.storage.tick(have_time);
    }

    pub fn check_consistency_with_blocks(&mut self, chunk_pos: ChunkPos, section_y: i32, block_section: &LevelChunkSection) {
        let key = section_key(chunk_pos.x, section_y, chunk_pos.z);
        let may_have_poi = block_section.maybe_has(poi_types::has_poi);
        match self.storage.get_or_load(key) {
            Some(section) => {
                if section.refresh(|output| {
                    if may_have_poi {
                        update_from_section(block_section, chunk_pos, section_y, output);
                    }
                }) {
                    self.storage.set_dirty(key);
                }
            }
            None => {
                if may_have_poi {
                    let mut added = false;
                    let section = self.storage.get_or_create(key);
                    update_from_section(block_section, chunk_pos, section_y, &mut |pos, poi_type| added |= section.add(pos, poi_type).is_some());
                    if added {
                        self.storage.set_dirty(key);
                    }
                }
            }
        }
    }

    pub fn update_on_block_state_change(&mut self, pos: BlockPos, old: crisp_data::BlockState, new: crisp_data::BlockState) {
        let (old_type, new_type) = (poi_types::for_state(old), poi_types::for_state(new));
        if old_type == new_type {
            return;
        }
        if old_type.is_some() {
            self.remove(pos);
        }
        if let Some(new_type) = new_type {
            self.add(pos, new_type);
        }
    }

    pub fn section_range(&self) -> (i32, i32) {
        self.storage.section_range()
    }

    pub fn needs_loading(&mut self, chunk_pos: ChunkPos, section_y: i32) -> bool {
        !self.storage.get_or_load(section_key(chunk_pos.x, section_y, chunk_pos.z)).is_some_and(|section| section.is_valid())
    }
}

pub fn ensure_loaded_and_valid(chunks: &mut ChunkCache, center: BlockPos, radius: i32) {
    let center_chunk = center.chunk();
    let chunk_radius = radius.div_euclid(16);
    let (min_section_y, max_section_y) = chunks.poi_manager.section_range();
    for z in center_chunk.z - chunk_radius..=center_chunk.z + chunk_radius {
        for y in min_section_y..=max_section_y {
            for x in center_chunk.x - chunk_radius..=center_chunk.x + chunk_radius {
                let pos = ChunkPos::new(x, z);
                if chunks.poi_manager.needs_loading(pos, y) && chunks.poi_manager.loaded_chunks.add(pos.pack()) {
                    chunks.get_chunk_at_status(pos, ChunkStatus::Empty);
                }
            }
        }
    }
}

fn update_from_section(block_section: &LevelChunkSection, chunk_pos: ChunkPos, section_y: i32, output: &mut dyn FnMut(BlockPos, PoiType)) {
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                let state = block_section.block_state(x, y, z);
                if let Some(poi_type) = poi_types::for_state(state) {
                    output(BlockPos::new(chunk_pos.min_block_x() + x as i32, (section_y << 4) + y as i32, chunk_pos.min_block_z() + z as i32), poi_type);
                }
            }
        }
    }
}
