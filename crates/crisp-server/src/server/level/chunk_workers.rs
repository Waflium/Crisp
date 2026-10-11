use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Condvar, LazyLock, Mutex};
use std::thread;

use crisp_core::ChunkPos;
use crisp_protocol::nbt::Nbt;

use crisp_data::{Block, Fluid};

use crate::world::level::chunk::{ChunkStructures, LevelChunk};
use crate::world::level::chunk::serialization::{self, ReadChunk, SectionLight};
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::ticks::SavedTick;

type Job = Box<dyn FnOnce() + Send>;

struct WorkerPool {
    jobs: Mutex<VecDeque<Job>>,
    available: Condvar,
}

static POOL: LazyLock<&'static WorkerPool> = LazyLock::new(|| {
    let pool: &'static WorkerPool = Box::leak(Box::new(WorkerPool { jobs: Mutex::new(VecDeque::new()), available: Condvar::new() }));
    let threads = thread::available_parallelism().map_or(1, |n| n.get()).saturating_sub(1).clamp(1, 255);
    for i in 0..threads {
        thread::Builder::new()
            .name(format!("Worker-Main-{}", i + 1))
            .spawn(move || {
                loop {
                    let job = {
                        let mut jobs = pool.jobs.lock().unwrap();
                        loop {
                            if let Some(job) = jobs.pop_front() {
                                break job;
                            }
                            jobs = pool.available.wait(jobs).unwrap();
                        }
                    };
                    job();
                }
            })
            .expect("worker thread");
    }
    pool
});

pub fn execute(job: impl FnOnce() + Send + 'static) {
    let pool = *POOL;
    pool.jobs.lock().unwrap().push_back(Box::new(job));
    pool.available.notify_one();
}

pub enum WorkResult {
    Loaded { pos: ChunkPos, token: u64, result: io::Result<Option<ReadChunk>> },
    Generated { pos: ChunkPos, token: u64, chunk: LevelChunk },
    StructureStarts { pos: ChunkPos, token: u64, structures: ChunkStructures },
    Wake,
}

pub struct ChunkToSave {
    pub chunk: LevelChunk,
    pub game_time: i64,
    pub block_ticks: Vec<SavedTick<Block>>,
    pub fluid_ticks: Vec<SavedTick<Fluid>>,
    pub light: Vec<SectionLight>,
}

enum IoRequest {
    Read { pos: ChunkPos, reply: Box<dyn FnOnce(io::Result<Option<Nbt>>) + Send> },
    Write { pos: ChunkPos, nbt: Nbt },
    Clear { pos: ChunkPos },
    WriteChunk(Box<ChunkToSave>),
    Flush { flush: bool, reply: Sender<io::Result<()>> },
}

pub struct IoWorker {
    requests: Sender<IoRequest>,
}

impl IoWorker {
    pub fn new(mut storage: RegionStorage) -> IoWorker {
        let (requests, incoming) = channel::<IoRequest>();
        thread::Builder::new()
            .name("IO-Worker".into())
            .spawn(move || {
                for request in incoming {
                    match request {
                        IoRequest::Read { pos, reply } => reply(storage.read(pos)),
                        IoRequest::Write { pos, nbt } => write(&mut storage, pos, &nbt),
                        IoRequest::Clear { pos } => {
                            if storage.clear(pos).is_err() {
                                error!("Failed to store chunk [{}, {}]", pos.x, pos.z);
                            }
                        }
                        IoRequest::WriteChunk(save) => {
                            let ChunkToSave { chunk, game_time, block_ticks, fluid_ticks, light } = *save;
                            write(&mut storage, chunk.pos, &serialization::write(&chunk, game_time, &block_ticks, &fluid_ticks, &light));
                        }
                        IoRequest::Flush { flush, reply } => drop(reply.send(if flush { storage.flush() } else { Ok(()) })),
                    }
                }
            })
            .expect("IO worker thread");
        IoWorker { requests }
    }

    pub fn read(&self, pos: ChunkPos, reply: impl FnOnce(io::Result<Option<Nbt>>) + Send + 'static) {
        let _ = self.requests.send(IoRequest::Read { pos, reply: Box::new(reply) });
    }

    pub fn write(&self, pos: ChunkPos, nbt: Nbt) {
        let _ = self.requests.send(IoRequest::Write { pos, nbt });
    }

    pub fn clear(&self, pos: ChunkPos) {
        let _ = self.requests.send(IoRequest::Clear { pos });
    }

    pub fn write_chunk(&self, save: ChunkToSave) {
        let _ = self.requests.send(IoRequest::WriteChunk(Box::new(save)));
    }

    pub fn flush(&self) -> io::Result<()> {
        self.synchronize(true)
    }

    pub fn synchronize(&self, flush: bool) -> io::Result<()> {
        let (reply, result) = channel();
        if self.requests.send(IoRequest::Flush { flush, reply }).is_err() {
            return Err(io::Error::other("IO worker gone"));
        }
        result.recv().unwrap_or_else(|_| Err(io::Error::other("IO worker gone")))
    }
}

fn write(storage: &mut RegionStorage, pos: ChunkPos, nbt: &Nbt) {
    if storage.write(pos, nbt).is_err() {
        error!("Failed to store chunk [{}, {}]", pos.x, pos.z);
        error!("Failed to save chunk {},{}", pos.x, pos.z);
    }
}

pub fn results() -> (Sender<WorkResult>, Receiver<WorkResult>) {
    channel()
}
