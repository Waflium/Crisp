use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use crisp_core::ChunkPos;
use crisp_core::hash_map::FastSet;
use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use super::RegionStorage;
use crate::server::level::ServerLevel;
use crate::server::level::chunk_workers::IoWorker;
use crate::world::entity::LevelEntity;

pub type EntityChunkRead = (ChunkPos, io::Result<Option<Nbt>>);

pub struct EntityStorage {
    worker: Option<IoWorker>,
    empty_chunks: FastSet<i64>,
    reads: (Sender<EntityChunkRead>, Receiver<EntityChunkRead>),
    loading_inbox: Arc<AtomicUsize>,
}

impl EntityStorage {
    pub fn new(storage: Option<RegionStorage>) -> EntityStorage {
        EntityStorage { worker: storage.map(IoWorker::new), empty_chunks: FastSet::default(), reads: channel(), loading_inbox: Arc::default() }
    }

    pub fn load_entities(&mut self, pos: ChunkPos) {
        let Some(worker) = self.worker.as_ref().filter(|_| !self.empty_chunks.contains(&pos.pack())) else {
            self.loading_inbox.fetch_add(1, Ordering::Relaxed);
            let _ = self.reads.0.send((pos, Ok(None)));
            return;
        };
        let reads = self.reads.0.clone();
        let loading_inbox = self.loading_inbox.clone();
        worker.read(pos, move |read| {
            loading_inbox.fetch_add(1, Ordering::Relaxed);
            drop(reads.send((pos, read)));
        });
    }

    pub fn poll_read(&mut self) -> Option<EntityChunkRead> {
        let read = self.reads.1.try_recv().ok()?;
        self.loading_inbox.fetch_sub(1, Ordering::Relaxed);
        Some(read)
    }

    pub fn loading_inbox_size(&self) -> usize {
        self.loading_inbox.load(Ordering::Relaxed)
    }

    pub fn mark_empty(&mut self, pos: ChunkPos) {
        self.empty_chunks.insert(pos.pack());
    }

    pub fn store_entities<'a>(&mut self, pos: ChunkPos, entities: impl ExactSizeIterator<Item = &'a LevelEntity>) {
        if entities.len() == 0 {
            if self.empty_chunks.insert(pos.pack())
                && let Some(worker) = &self.worker
            {
                worker.clear(pos);
            }
        } else {
            if let Some(worker) = &self.worker {
                worker.write(pos, chunk_tag(pos, entities));
            }
            self.empty_chunks.remove(&pos.pack());
        }
    }

    pub fn flush(&self, flush_storage: bool) {
        if let Some(worker) = &self.worker
            && let Err(e) = worker.synchronize(flush_storage)
        {
            warn!("Failed to synchronize entity chunks: {e}");
        }
    }
}

pub fn chunk_tag<'a>(pos: ChunkPos, entities: impl Iterator<Item = &'a LevelEntity>) -> Nbt {
    Nbt::Compound(vec![
        ("DataVersion".into(), Nbt::Int(WORLD_VERSION)),
        ("Entities".into(), Nbt::List(entities.filter_map(LevelEntity::save).collect())),
        ("Position".into(), Nbt::IntArray(vec![pos.x, pos.z])),
    ])
}

impl ServerLevel {
    pub fn read_chunk_entities(&mut self, pos: ChunkPos, tag: &Nbt) -> Vec<LevelEntity> {
        match tag.get("Position") {
            Some(Nbt::IntArray(stored)) if stored.len() == 2 => {
                if *stored != [pos.x, pos.z] {
                    let (x, z) = (pos.x, pos.z);
                    error!("Chunk file at [{x}, {z}] is in the wrong location. (Expected [{x}, {z}], got [{}, {}])", stored[0], stored[1]);
                }
            }
            _ => warn!("Failed to parse chunk [{}, {}] position info", pos.x, pos.z),
        }
        let entities = tag.get("Entities").map_or(&[][..], Nbt::as_list);
        entities.iter().filter_map(|e| self.load_entity(e)).collect()
    }
}
