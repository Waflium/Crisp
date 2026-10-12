use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use crisp_core::ChunkPos;
use crisp_core::fastutil::{LongLinkedOpenHashSet, LongOpenHashSet};
use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use crate::server::level::chunk_workers::IoWorker;

pub trait SectionData: Default + Send + 'static {
    fn pack(&self) -> Nbt;
    fn unpack(nbt: &Nbt) -> Option<Self>;
}

pub struct PackedChunk<R> {
    sections_by_y: Vec<(i32, R)>,
    version_changed: bool,
}

impl<R: SectionData> PackedChunk<R> {
    fn parse(tag: &Nbt, min_section_y: i32, max_section_y: i32) -> PackedChunk<R> {
        let version_changed = tag.get("DataVersion").and_then(Nbt::as_i64).is_none_or(|v| (v as i32) < WORLD_VERSION);
        let sections = tag.get("Sections");
        let mut sections_by_y = Vec::new();
        for section_y in min_section_y..=max_section_y {
            if let Some(section) = sections.and_then(|s| s.get(&section_y.to_string())).and_then(R::unpack) {
                sections_by_y.push((section_y, section));
            }
        }
        PackedChunk { sections_by_y, version_changed }
    }
}

pub fn section_key(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FFFFF) << 42) | (y as i64 & 0xFFFFF) | ((z as i64 & 0x3FFFFF) << 20)
}

fn section_x(key: i64) -> i32 {
    (key >> 42) as i32
}

fn section_y(key: i64) -> i32 {
    ((key << 44) >> 44) as i32
}

fn section_z(key: i64) -> i32 {
    ((key << 22) >> 42) as i32
}

pub struct SectionStorage<R> {
    io: Option<IoWorker>,
    storage: HashMap<i64, Option<R>>,
    dirty_chunks: LongLinkedOpenHashSet,
    min_section_y: i32,
    max_section_y: i32,
    loaded_chunks: LongOpenHashSet,
    pending_loads: HashMap<i64, Receiver<Option<PackedChunk<R>>>>,
}

impl<R: SectionData> SectionStorage<R> {
    pub fn new(io: Option<IoWorker>, min_section_y: i32, max_section_y: i32) -> SectionStorage<R> {
        SectionStorage {
            io,
            storage: HashMap::new(),
            dirty_chunks: LongLinkedOpenHashSet::new(),
            min_section_y,
            max_section_y,
            loaded_chunks: LongOpenHashSet::new(),
            pending_loads: HashMap::new(),
        }
    }

    pub fn section_range(&self) -> (i32, i32) {
        (self.min_section_y, self.max_section_y)
    }

    pub fn tick(&mut self, have_time: &mut dyn FnMut() -> bool) {
        while have_time() {
            let Some(chunk) = self.dirty_chunks.remove_first() else { break };
            self.write_chunk(ChunkPos::unpack(chunk));
        }
        self.unpack_pending_loads();
    }

    fn unpack_pending_loads(&mut self) {
        let ready: Vec<(i64, Option<PackedChunk<R>>)> =
            self.pending_loads.iter().filter_map(|(&key, receiver)| receiver.try_recv().ok().map(|chunk| (key, chunk))).collect();
        for (key, chunk) in ready {
            self.pending_loads.remove(&key);
            self.unpack_packed_chunk(ChunkPos::unpack(key), chunk);
            self.loaded_chunks.add(key);
        }
    }

    pub fn flush_all(&mut self) {
        while let Some(chunk) = self.dirty_chunks.remove_first() {
            self.write_chunk(ChunkPos::unpack(chunk));
        }
    }

    pub fn has_work(&self) -> bool {
        !self.dirty_chunks.is_empty()
    }

    pub fn get(&self, section_pos: i64) -> Option<&Option<R>> {
        self.storage.get(&section_pos)
    }

    pub fn get_mut(&mut self, section_pos: i64) -> Option<&mut Option<R>> {
        self.storage.get_mut(&section_pos)
    }

    pub fn get_or_load(&mut self, section_pos: i64) -> Option<&mut R> {
        if self.outside_stored_range(section_pos) {
            return None;
        }
        if !self.storage.contains_key(&section_pos) {
            self.unpack_chunk(ChunkPos::new(section_x(section_pos), section_z(section_pos)));
        }
        self.storage.get_mut(&section_pos).expect("unpacked section").as_mut()
    }

    fn outside_stored_range(&self, section_pos: i64) -> bool {
        let y = section_y(section_pos);
        y < self.min_section_y || y > self.max_section_y
    }

    pub fn get_or_create(&mut self, section_pos: i64) -> &mut R {
        assert!(!self.outside_stored_range(section_pos), "sectionPos out of bounds");
        if self.get_or_load(section_pos).is_none() {
            self.storage.insert(section_pos, Some(R::default()));
        }
        self.storage.get_mut(&section_pos).and_then(Option::as_mut).expect("created section")
    }

    pub fn prefetch(&mut self, chunk_pos: ChunkPos) {
        let key = chunk_pos.pack();
        if self.loaded_chunks.contains(key) || self.pending_loads.contains_key(&key) {
            return;
        }
        let receiver = self.try_read(chunk_pos);
        self.pending_loads.insert(key, receiver);
    }

    fn unpack_chunk(&mut self, chunk_pos: ChunkPos) {
        let key = chunk_pos.pack();
        if !self.loaded_chunks.add(key) {
            return;
        }
        let receiver = match self.pending_loads.remove(&key) {
            Some(receiver) => receiver,
            None => self.try_read(chunk_pos),
        };
        let chunk = receiver.recv().unwrap_or(None);
        self.unpack_packed_chunk(chunk_pos, chunk);
    }

    fn try_read(&self, chunk_pos: ChunkPos) -> Receiver<Option<PackedChunk<R>>> {
        let (sender, receiver) = channel();
        let (min_section_y, max_section_y) = (self.min_section_y, self.max_section_y);
        match &self.io {
            Some(io) => io.read(chunk_pos, move |read| {
                let chunk = match read {
                    Ok(tag) => tag.map(|tag| PackedChunk::parse(&tag, min_section_y, max_section_y)),
                    Err(_) => {
                        error!("Error reading chunk [{}, {}] data from disk", chunk_pos.x, chunk_pos.z);
                        None
                    }
                };
                let _ = sender.send(chunk);
            }),
            None => {
                let _ = sender.send(None);
            }
        }
        receiver
    }

    fn unpack_packed_chunk(&mut self, pos: ChunkPos, packed_chunk: Option<PackedChunk<R>>) {
        match packed_chunk {
            None => {
                for section_y in self.min_section_y..=self.max_section_y {
                    self.storage.insert(section_key(pos.x, section_y, pos.z), None);
                }
            }
            Some(PackedChunk { mut sections_by_y, version_changed }) => {
                for section_y in self.min_section_y..=self.max_section_y {
                    let key = section_key(pos.x, section_y, pos.z);
                    let section = sections_by_y.iter().position(|(y, _)| *y == section_y).map(|i| sections_by_y.swap_remove(i).1);
                    let present = section.is_some();
                    self.storage.insert(key, section);
                    if present && version_changed {
                        self.set_dirty(key);
                    }
                }
            }
        }
    }

    fn write_chunk(&mut self, chunk_pos: ChunkPos) {
        let mut sections = Vec::new();
        for section_y in self.min_section_y..=self.max_section_y {
            if let Some(Some(section)) = self.storage.get(&section_key(chunk_pos.x, section_y, chunk_pos.z)) {
                sections.push((section_y.to_string(), section.pack()));
            }
        }
        let tag = Nbt::Compound(vec![("Sections".into(), Nbt::Compound(sections)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        if let Some(io) = &self.io {
            io.write(chunk_pos, tag);
        }
    }

    pub fn set_dirty(&mut self, section_pos: i64) {
        if self.storage.get(&section_pos).is_some_and(Option::is_some) {
            self.dirty_chunks.add(ChunkPos::new(section_x(section_pos), section_z(section_pos)).pack());
        } else {
            warn!("No data for position: SectionPos{{x={}, y={}, z={}}}", section_x(section_pos), section_y(section_pos), section_z(section_pos));
        }
    }

    pub fn flush(&mut self, chunk_pos: ChunkPos) {
        if self.dirty_chunks.remove(chunk_pos.pack()) {
            self.write_chunk(chunk_pos);
        }
    }

    pub fn synchronize(&self, flush: bool) {
        if let Some(io) = &self.io {
            let _ = io.synchronize(flush);
        }
    }
}
