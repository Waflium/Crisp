use crisp_core::hash_map::FastMap;
use std::collections::{HashMap, VecDeque};

use indexmap::{IndexMap, IndexSet};

use crisp_core::ChunkPos;
use crisp_core::fastutil::LongOpenHashSet;

use crisp_data::{Block, Fluid};

use super::chunk_level::{self, ChunkStatus, FullChunkStatus};
use crate::world::level::chunk::LevelChunk;
use crate::world::ticks::LevelChunkTicks;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FutureId(u64);

pub const UNLOADED: FutureId = FutureId(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Waiter {
    Task(u64),
    Range(u64),
    ReleasePlayerTicket(i64),
    FullStatus { future: FutureId, node: i64, holder: u64, token: u64, status: FullChunkStatus },
}

struct Future {
    result: Option<bool>,
    waiters: Vec<Waiter>,
}

#[derive(Default)]
pub struct Futures {
    futures: HashMap<FutureId, Future>,
    next: u64,
    pub woken: VecDeque<Waiter>,
}

impl Futures {
    pub fn new_pending(&mut self) -> FutureId {
        self.next += 1;
        let id = FutureId(self.next);
        self.futures.insert(id, Future { result: None, waiters: Vec::new() });
        id
    }

    pub fn new_done(&mut self, success: bool) -> FutureId {
        let id = self.new_pending();
        self.futures.get_mut(&id).unwrap().result = Some(success);
        id
    }

    pub fn result(&self, id: FutureId) -> Option<bool> {
        if id == UNLOADED {
            return Some(false);
        }
        self.futures.get(&id).and_then(|f| f.result)
    }

    pub fn is_done(&self, id: FutureId) -> bool {
        self.result(id).is_some()
    }

    pub fn is_success(&self, id: FutureId) -> bool {
        self.result(id) == Some(true)
    }

    pub fn complete(&mut self, id: FutureId, success: bool) -> bool {
        let Some(future) = self.futures.get_mut(&id) else { return false };
        if future.result.is_some() {
            return false;
        }
        future.result = Some(success);
        self.woken.extend(future.waiters.drain(..));
        true
    }

    pub fn then(&mut self, id: FutureId, waiter: Waiter) {
        match self.futures.get_mut(&id) {
            Some(future) if future.result.is_none() => future.waiters.push(waiter),
            _ => self.woken.push_back(waiter),
        }
    }
}

pub struct ChunkHolder {
    pub pos: ChunkPos,
    pub id: u64,
    pub highest_allowed_status: Option<ChunkStatus>,
    pub started_work: Option<ChunkStatus>,
    pub futures: [Option<FutureId>; 6],
    pub task: Option<u64>,
    pub generation_ref_count: u32,
    pub persisted_status: Option<ChunkStatus>,
    pub loaded: Option<Box<LevelChunk>>,
    pub saved_ticks: Option<Box<(LevelChunkTicks<Block>, LevelChunkTicks<Fluid>)>>,
    pub old_ticket_level: i32,
    pub ticket_level: i32,
    pub queue_level: i32,
    pub full_chunk_future: FutureId,
    pub ticking_chunk_future: FutureId,
    pub entity_ticking_chunk_future: FutureId,
    pub was_accessible_since_last_save: bool,
    pub pending_full_state_confirmation: u64,
}

impl ChunkHolder {
    fn new(pos: ChunkPos, id: u64, ticket_level: i32) -> ChunkHolder {
        let level = chunk_level::MAX_LEVEL + 1;
        ChunkHolder {
            pos,
            id,
            highest_allowed_status: None,
            started_work: None,
            futures: [None; 6],
            task: None,
            generation_ref_count: 0,
            persisted_status: None,
            loaded: None,
            saved_ticks: None,
            old_ticket_level: level,
            ticket_level,
            queue_level: level,
            full_chunk_future: UNLOADED,
            ticking_chunk_future: UNLOADED,
            entity_ticking_chunk_future: UNLOADED,
            was_accessible_since_last_save: false,
            pending_full_state_confirmation: 0,
        }
    }

    pub fn full_status(&self) -> FullChunkStatus {
        chunk_level::full_status(self.ticket_level)
    }

    pub fn is_status_disallowed(&self, status: ChunkStatus) -> bool {
        self.highest_allowed_status.is_none_or(|h| status > h)
    }

    pub fn acquire_status_bump(&mut self, status: ChunkStatus) -> bool {
        let parent = if status == ChunkStatus::Empty { None } else { Some(status.parent()) };
        if self.started_work == parent {
            self.started_work = Some(status);
            true
        } else if self.started_work.is_some_and(|s| status <= s) {
            false
        } else {
            panic!("Unexpected last startedWork status: {:?} while trying to start: {status:?}", self.started_work);
        }
    }

    pub fn status_done(&self, futures: &Futures, status: ChunkStatus) -> bool {
        self.futures[status.index()].is_some_and(|f| futures.is_success(f))
    }

    pub fn is_ticking(&self, futures: &Futures) -> bool {
        futures.is_success(self.ticking_chunk_future)
    }

    pub fn is_ready_for_saving(&self, futures: &Futures) -> bool {
        self.generation_ref_count == 0
            && [self.full_chunk_future, self.ticking_chunk_future, self.entity_ticking_chunk_future].iter().all(|&f| futures.is_done(f))
    }

    pub fn referenced_futures(&self) -> impl Iterator<Item = FutureId> + '_ {
        self.futures.iter().flatten().copied().chain([self.full_chunk_future, self.ticking_chunk_future, self.entity_ticking_chunk_future])
    }
}

#[derive(Default)]
pub struct ChunkHolders {
    pub futures: Futures,
    pub tasks: HashMap<u64, ChunkGenerationTask>,
    next_task: u64,
    pub pending_generation_tasks: Vec<u64>,
    pub ranges: HashMap<u64, RangeWait>,
    next_range: u64,
    pub map: FastMap<i64, ChunkHolder>,
    pub pending_unloads: IndexMap<i64, ChunkHolder>,
    pub to_drop: LongOpenHashSet,
    pub chunks_to_update_futures: IndexSet<i64>,
    pub modified: bool,
    next_id: u64,
}

impl ChunkHolders {
    pub fn add_task(&mut self, task: ChunkGenerationTask) -> u64 {
        self.next_task += 1;
        self.tasks.insert(self.next_task, task);
        self.next_task
    }

    pub fn add_range(&mut self, range: RangeWait) -> u64 {
        self.next_range += 1;
        self.ranges.insert(self.next_range, range);
        self.next_range
    }

    pub fn collect_garbage(&mut self) {
        let mut live: std::collections::HashSet<FutureId> = std::collections::HashSet::new();
        for holder in self.map.values().chain(self.pending_unloads.values()) {
            live.extend(holder.referenced_futures());
        }
        for task in self.tasks.values() {
            live.extend(task.scheduled_layer.iter().copied());
        }
        for range in self.ranges.values() {
            live.extend(range.deps.iter().copied());
            let (RangeThen::Complete(f) | RangeThen::Ticking { future: f, .. }) = range.then;
            live.insert(f);
        }
        self.futures.futures.retain(|id, f| live.contains(id) || f.result.is_none() || !f.waiters.is_empty());
    }

    pub fn future_count(&self) -> usize {
        self.futures.futures.len()
    }

    pub fn get(&self, node: i64) -> Option<&ChunkHolder> {
        self.map.get(&node).or_else(|| self.pending_unloads.get(&node))
    }

    pub fn get_mut(&mut self, node: i64) -> Option<&mut ChunkHolder> {
        match self.map.get_mut(&node) {
            Some(holder) => Some(holder),
            None => self.pending_unloads.get_mut(&node),
        }
    }

    pub fn update_chunk_scheduling(&mut self, node: i64, level: i32, old_level: i32) -> bool {
        let exists = self.map.contains_key(&node);
        if !chunk_level::is_loaded(old_level) && !chunk_level::is_loaded(level) {
            return exists;
        }
        if let Some(holder) = self.map.get_mut(&node) {
            holder.ticket_level = level;
            if !chunk_level::is_loaded(level) {
                self.to_drop.add(node);
            } else {
                self.to_drop.remove(node);
            }
        }
        if chunk_level::is_loaded(level) && !exists {
            let holder = match self.pending_unloads.shift_remove(&node) {
                Some(mut holder) => {
                    holder.ticket_level = level;
                    holder
                }
                None => {
                    self.next_id += 1;
                    ChunkHolder::new(ChunkPos::unpack(node), self.next_id, level)
                }
            };
            self.map.insert(node, holder);
            self.modified = true;
        }
        self.map.contains_key(&node)
    }
}

pub struct ChunkGenerationTask {
    pub pos: ChunkPos,
    pub target_status: ChunkStatus,
    pub scheduled_status: Option<ChunkStatus>,
    pub marked_for_cancellation: bool,
    pub scheduled_layer: Vec<FutureId>,
    pub radius: i32,
    pub needs_generation: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RangeThen {
    Complete(FutureId),
    Ticking { pos: ChunkPos, future: FutureId },
}

pub struct RangeWait {
    pub deps: Vec<FutureId>,
    pub remaining: usize,
    pub then: RangeThen,
}
