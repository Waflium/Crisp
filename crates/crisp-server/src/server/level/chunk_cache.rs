use crisp_core::hash_map::{FastMap, FastSet};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use indexmap::IndexSet;

use crisp_core::{BlockPos, ChunkPos};
use crisp_data::{Block, BlockState, DimensionType, Fluid, blocks};
use crisp_protocol::Writer;

use super::chunk_holder::{ChunkGenerationTask, ChunkHolders, FutureId, RangeThen, RangeWait, UNLOADED, Waiter};
use super::chunk_level::{self, ChunkStatus, FullChunkStatus};
use super::chunk_task_dispatcher::ChunkTaskPriorityQueue;
use super::chunk_workers::{self, ChunkToSave, IoWorker, WorkResult};
use super::distance_manager::DistanceManager;
use super::threaded_level_light_engine::{LightTask, TaskType, ThreadedLevelLightEngine};
use super::ticket::{Ticket, TicketType};
use crate::world::difficulty::Difficulty;
use crate::world::level::BlockGetter;
use crate::world::level::chunk::serialization::{self, SectionLight};
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::chunk::block_entities::{BlockEntityTickers, EntityCreationType};
use super::world_gen_region::{RegionLightTask, WorldGenRegion};
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::chunk::{Biome, ChunkGenerator, ChunkStructures, HeightmapType, LevelChunk};
use crate::world::level::levelgen::beardifier::Beardifier;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::terrain_adjustment::TerrainAdjustment;
use crate::world::level::levelgen::structure::structure_start::StructureStart;
use std::sync::Arc;
use crate::world::level::levelgen::material::WorldGenerationContext;
use crate::world::level::levelgen::random_state::RandomState;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::lighting::{self, section};
use crate::world::ticks::{LevelChunkTicks, LevelTicks};

struct Job {
    pos: ChunkPos,
    holder: u64,
    status: ChunkStatus,
    work: FutureId,
}

pub struct ChunkCache {
    pub dimension: &'static DimensionType,
    pub generator: ChunkGenerator,
    chunks: FastMap<ChunkPos, LevelChunk>,
    terrain_chunks: FastMap<ChunkPos, LevelChunk>,
    structure_chunks: FastMap<ChunkPos, ChunkStructures>,
    loaded_structure_chunks: FastSet<ChunkPos>,
    structure_state: Option<Arc<ChunkGeneratorStructureState>>,
    region_random_state: Option<Arc<RandomState>>,
    pub generate_structures: bool,
    pub seed: i64,
    pub difficulty: Difficulty,
    io: Option<IoWorker>,
    pub light_engine: ThreadedLevelLightEngine,
    light_changes: Vec<(LightLayer, i64)>,
    ticking_light_changes: Vec<(LightLayer, i64)>,
    pub block_ticks: LevelTicks<Block>,
    pub fluid_ticks: LevelTicks<Fluid>,
    pub game_time: i64,
    pub distance_manager: DistanceManager,
    holders: ChunkHolders,
    worldgen_queue: ChunkTaskPriorityQueue<u64>,
    results: (Sender<WorkResult>, Receiver<WorkResult>),
    jobs: HashMap<u64, Job>,
    next_token: u64,
    in_flight: usize,
    light_done: Vec<u64>,
    unload_queue: VecDeque<i64>,
    waiting_unloads: IndexSet<i64>,
    ready_to_send: Vec<ChunkPos>,
    ticking_ready: Vec<ChunkPos>,
    full_status_changes: VecDeque<(ChunkPos, FullChunkStatus)>,
    last_chunk_pos: [Option<ChunkPos>; 4],
    server_view_distance: i32,
    live_futures: usize,
    pub block_entity_tickers: BlockEntityTickers,
    pub tick_deadline: Option<Instant>,
}

impl ChunkCache {
    pub fn new(dimension: &'static DimensionType, generator: impl Into<ChunkGenerator>, storage: Option<RegionStorage>) -> ChunkCache {
        let generator = generator.into();
        let seed = match &generator {
            ChunkGenerator::Noise(noise) => noise.seed(),
            ChunkGenerator::Flat(_) => 0,
        };
        let structure_state = None;
        let mut cache = ChunkCache {
            dimension,
            generator,
            chunks: FastMap::default(),
            terrain_chunks: FastMap::default(),
            structure_chunks: FastMap::default(),
            loaded_structure_chunks: FastSet::default(),
            structure_state,
            region_random_state: None,
            generate_structures: true,
            seed: 0,
            difficulty: Difficulty::Easy,
            io: storage.map(IoWorker::new),
            light_engine: ThreadedLevelLightEngine::new(dimension.min_y, dimension.height as i32, dimension.has_skylight),
            light_changes: Vec::new(),
            ticking_light_changes: Vec::new(),
            block_ticks: LevelTicks::default(),
            fluid_ticks: LevelTicks::default(),
            game_time: 0,
            distance_manager: DistanceManager::default(),
            holders: ChunkHolders::default(),
            worldgen_queue: ChunkTaskPriorityQueue::default(),
            results: chunk_workers::results(),
            jobs: HashMap::new(),
            next_token: 0,
            in_flight: 0,
            light_done: Vec::new(),
            unload_queue: VecDeque::new(),
            waiting_unloads: IndexSet::new(),
            ready_to_send: Vec::new(),
            ticking_ready: Vec::new(),
            full_status_changes: VecDeque::new(),
            last_chunk_pos: [None; 4],
            server_view_distance: 0,
            live_futures: 0,
            tick_deadline: None,
            block_entity_tickers: BlockEntityTickers::default(),
        };
        cache.set_seed(seed);
        cache.set_view_distance(10);
        cache
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = seed;
        let structure_state = match &self.generator {
            ChunkGenerator::Noise(_) => ChunkGeneratorStructureState::create_for_normal(&self.generator, seed),
            ChunkGenerator::Flat(flat) => ChunkGeneratorStructureState::create_for_flat(flat, seed),
        };
        self.structure_state = Some(Arc::new(structure_state));
        self.region_random_state = match &self.generator {
            ChunkGenerator::Noise(_) => None,
            ChunkGenerator::Flat(_) => Some(Arc::new(RandomState::dummy(seed))),
        };
    }

    pub fn min_y(&self) -> i32 {
        self.dimension.min_y
    }

    pub fn height(&self) -> i32 {
        self.dimension.height as i32
    }

    pub fn max_y(&self) -> i32 {
        self.min_y() + self.height() - 1
    }

    pub fn is_outside_build_height(&self, y: i32) -> bool {
        y < self.min_y() || y > self.max_y()
    }

    pub fn set_view_distance(&mut self, view_distance: i32) {
        let view_distance = view_distance.clamp(2, 32);
        if view_distance != self.server_view_distance {
            self.server_view_distance = view_distance;
            self.distance_manager.update_player_tickets(view_distance);
        }
    }

    pub fn set_simulation_distance(&mut self, simulation_distance: i32) {
        self.distance_manager.update_simulation_distance(&mut self.holders, simulation_distance);
    }

    pub fn add_ticket(&mut self, ticket: Ticket, pos: ChunkPos) {
        self.distance_manager.add_ticket(&mut self.holders, pos.pack(), ticket);
    }

    pub fn remove_ticket(&mut self, ticket: Ticket, pos: ChunkPos) {
        self.distance_manager.remove_ticket(&mut self.holders, pos.pack(), ticket);
    }

    pub fn add_ticket_with_radius(&mut self, ty: TicketType, pos: ChunkPos, radius: i32) {
        self.distance_manager.add_ticket_with_radius(&mut self.holders, ty, pos, radius);
    }

    pub fn remove_ticket_with_radius(&mut self, ty: TicketType, pos: ChunkPos, radius: i32) {
        self.distance_manager.remove_ticket_with_radius(&mut self.holders, ty, pos, radius);
    }

    pub fn update_chunk_forced(&mut self, pos: ChunkPos, forced: bool) -> bool {
        self.distance_manager.update_chunk_forced(&mut self.holders, pos, forced)
    }

    pub fn force_loaded_chunks(&self) -> &crisp_core::fastutil::LongOpenHashSet {
        self.distance_manager.ticket_storage.force_loaded_chunks()
    }

    pub fn activate_all_deactivated_tickets(&mut self) {
        self.distance_manager.activate_all_deactivated_tickets(&mut self.holders);
    }

    pub fn deactivate_tickets_on_closing(&mut self) {
        self.distance_manager.deactivate_tickets_on_closing(&mut self.holders);
    }

    pub fn add_player(&mut self, chunk: ChunkPos, player: crate::world::entity::EntityId) {
        self.distance_manager.add_player(&mut self.holders, chunk, player);
    }

    pub fn remove_player(&mut self, chunk: ChunkPos, player: crate::world::entity::EntityId) {
        self.distance_manager.remove_player(&mut self.holders, chunk, player);
    }

    pub fn has_active_tickets(&self) -> bool {
        self.distance_manager.ticket_storage.should_keep_dimension_active()
    }

    pub fn ticket_level(&self, pos: ChunkPos) -> Option<i32> {
        self.holders.map.get(&pos.pack()).map(|h| h.ticket_level)
    }

    pub fn loaded_chunks_count(&self) -> usize {
        self.holders.map.len()
    }

    fn chunk_absent(&self, key: i64, target_level: i32) -> bool {
        self.holders.map.get(&key).is_none_or(|h| h.ticket_level > target_level)
    }

    pub fn has_chunk(&self, pos: ChunkPos) -> bool {
        !self.chunk_absent(pos.pack(), chunk_level::by_status(ChunkStatus::Full))
    }

    pub fn chunks_with_at_least_full_status(&self) -> Vec<i64> {
        let level = chunk_level::by_status(ChunkStatus::Full);
        self.holders.map.iter().filter(|(_, h)| h.ticket_level <= level).map(|(&key, _)| key).collect()
    }

    pub fn latest_status_is_full(&self, key: i64) -> bool {
        self.holders.map.get(&key).is_some_and(|h| h.status_done(&self.holders.futures, ChunkStatus::Full))
    }

    fn is_full(&self, key: i64) -> bool {
        self.holders.map.get(&key).is_some_and(|h| !h.is_status_disallowed(ChunkStatus::Full) && h.status_done(&self.holders.futures, ChunkStatus::Full))
    }

    pub fn loaded_chunk(&self, pos: ChunkPos) -> Option<&LevelChunk> {
        if self.is_full(pos.pack()) { self.chunks.get(&pos) } else { None }
    }

    pub fn chunk(&mut self, pos: ChunkPos) -> &mut LevelChunk {
        if !self.last_chunk_pos.contains(&Some(pos)) {
            let future = self.chunk_future_main_thread(pos, ChunkStatus::Full, true);
            self.managed_block(future);
            if !self.holders.futures.is_success(future) {
                panic!("Chunk not there when requested: {pos:?}");
            }
            self.last_chunk_pos.copy_within(0..3, 1);
            self.last_chunk_pos[0] = Some(pos);
        }
        self.chunks.get_mut(&pos).expect("full chunk")
    }

    fn chunk_future_main_thread(&mut self, pos: ChunkPos, status: ChunkStatus, load_or_generate: bool) -> FutureId {
        let key = pos.pack();
        let target_level = chunk_level::by_status(status);
        if load_or_generate {
            self.add_ticket(Ticket::new(TicketType::Unknown, target_level), pos);
            if self.chunk_absent(key, target_level) {
                self.run_distance_manager_updates();
                if self.chunk_absent(key, target_level) {
                    panic!("No chunk holder after ticket has been added: {pos:?}");
                }
            }
        }
        if self.chunk_absent(key, target_level) { UNLOADED } else { self.schedule_chunk_generation_task(key, status) }
    }

    fn managed_block(&mut self, future: FutureId) {
        while !self.holders.futures.is_done(future) {
            if !self.poll_task() {
                assert!(self.in_flight > 0, "waiting for a chunk with nothing to wait for");
                let result = self.results.1.recv().expect("result channel");
                self.apply_work_result(result);
            }
        }
    }

    pub fn prepare_ticking_chunk(&mut self, pos: ChunkPos) {
        self.add_ticket(Ticket::new(TicketType::PlayerLoading, chunk_level::BLOCK_TICKING_LEVEL), pos);
        self.run_distance_manager_updates();
        let future = self.holders.map[&pos.pack()].ticking_chunk_future;
        self.managed_block(future);
    }

    pub fn run_until_idle(&mut self) {
        loop {
            while self.poll_task() {}
            if self.in_flight == 0 {
                return;
            }
            let result = self.results.1.recv().expect("result channel");
            self.apply_work_result(result);
        }
    }

    pub fn is_ticking(&self, pos: ChunkPos) -> bool {
        self.holders.map.get(&pos.pack()).is_some_and(|h| h.is_ticking(&self.holders.futures))
    }

    pub fn chunk_to_send(&self, pos: ChunkPos) -> bool {
        self.is_ticking(pos)
    }

    pub fn is_position_ticking(&self, pos: ChunkPos) -> bool {
        is_position_ticking(&self.distance_manager, &self.holders, pos)
    }

    pub fn take_ticking_ready(&mut self) -> Vec<ChunkPos> {
        std::mem::take(&mut self.ticking_ready)
    }

    pub fn start_ticking_chunk(&mut self, pos: ChunkPos) {
        self.block_ticks.unpack_container(pos, self.game_time);
        self.fluid_ticks.unpack_container(pos, self.game_time);
        self.ready_to_send.push(pos);
    }

    pub fn take_ready_to_send(&mut self) -> Vec<ChunkPos> {
        std::mem::take(&mut self.ready_to_send)
    }

    pub fn pop_full_status_change(&mut self) -> Option<(ChunkPos, FullChunkStatus)> {
        self.full_status_changes.pop_front()
    }

    pub fn chunk_if_present(&mut self, pos: ChunkPos) -> Option<&mut LevelChunk> {
        self.chunks.get_mut(&pos)
    }

    pub fn collect_block_ticks(&mut self, game_time: i64, max: usize, entities_loaded: impl Fn(ChunkPos) -> bool) -> Vec<crate::world::ticks::ScheduledTick<Block>> {
        let ChunkCache { block_ticks, distance_manager, holders, .. } = self;
        block_ticks.collect(game_time, max, |c| entities_loaded(c) && is_position_ticking(distance_manager, holders, c))
    }

    pub fn collect_fluid_ticks(&mut self, game_time: i64, max: usize, entities_loaded: impl Fn(ChunkPos) -> bool) -> Vec<crate::world::ticks::ScheduledTick<Fluid>> {
        let ChunkCache { fluid_ticks, distance_manager, holders, .. } = self;
        fluid_ticks.collect(game_time, max, |c| entities_loaded(c) && is_position_ticking(distance_manager, holders, c))
    }

    pub fn block_ticking_chunks(&self) -> Vec<ChunkPos> {
        self.distance_manager.entity_ticking_chunks().into_iter().map(ChunkPos::unpack).filter(|&p| self.is_ticking(p)).collect()
    }

    pub fn purge_stale_tickets(&mut self) {
        self.distance_manager.purge_stale_tickets(&mut self.holders);
    }

    pub fn has_work(&self) -> bool {
        self.light_engine.has_work()
            || !self.holders.pending_unloads.is_empty()
            || !self.holders.map.is_empty()
            || !self.holders.to_drop.is_empty()
            || !self.unload_queue.is_empty()
            || !self.waiting_unloads.is_empty()
            || self.in_flight > 0
            || !self.holders.tasks.is_empty()
            || self.distance_manager.has_tickets()
    }

    pub fn tick_unloads(&mut self, no_save: bool, have_time: bool) {
        if !no_save {
            self.process_unloads(have_time);
        }
        self.clear_cache();
        let count = self.holders.future_count();
        if count > 4096 && count > 2 * self.live_futures {
            self.holders.collect_garbage();
            self.live_futures = self.holders.future_count();
        }
    }

    fn clear_cache(&mut self) {
        self.last_chunk_pos = [None; 4];
    }

    pub fn poll_task(&mut self) -> bool {
        if self.process_woken() {
            return true;
        }
        if self.run_distance_manager_updates() {
            return true;
        }
        if self.light_engine.has_work() {
            self.run_light_batch();
            return true;
        }
        if let Ok(result) = self.results.1.try_recv() {
            self.apply_work_result(result);
            return true;
        }
        if let Some(task) = self.distance_manager.main_thread_tasks.pop_front() {
            self.distance_manager.run_main_thread_task(&mut self.holders, task);
            return true;
        }
        self.run_worldgen()
    }

    pub fn waker(&self) -> Sender<WorkResult> {
        self.results.0.clone()
    }

    pub fn wait_for_work(&mut self, timeout: Duration) {
        if self.poll_task() {
            return;
        }
        match self.results.1.recv_timeout(timeout) {
            Ok(result) => self.apply_work_result(result),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
        }
    }

    fn process_woken(&mut self) -> bool {
        let mut any = false;
        while let Some(waiter) = self.holders.futures.woken.pop_front() {
            any = true;
            match waiter {
                Waiter::Task(task) => self.run_generation_task(task),
                Waiter::Range(range) => {
                    let wait = self.holders.ranges.get_mut(&range).expect("range");
                    wait.remaining -= 1;
                    if wait.remaining == 0 {
                        self.finish_range(range);
                    }
                }
                Waiter::ReleasePlayerTicket(pos) => {
                    self.distance_manager.main_thread_tasks.push_back(super::chunk_task_dispatcher::MainThreadTask::ReleasePlayerTicket(pos))
                }
                Waiter::FullStatus { future, node, holder, token, status } => {
                    let confirmed = self.holders.get(node).is_some_and(|h| h.id == holder && h.pending_full_state_confirmation == token);
                    if confirmed && self.holders.futures.is_success(future) {
                        self.full_status_changes.push_back((ChunkPos::unpack(node), status));
                    }
                }
            }
        }
        any
    }

    pub fn run_distance_manager_updates(&mut self) -> bool {
        let updated = self.distance_manager.run_tracker_updates(&mut self.holders);
        let result = if !self.holders.chunks_to_update_futures.is_empty() {
            let nodes: Vec<i64> = std::mem::take(&mut self.holders.chunks_to_update_futures).into_iter().collect();
            for &node in &nodes {
                self.update_highest_allowed_status(node);
            }
            for &node in &nodes {
                self.update_futures(node);
            }
            true
        } else {
            for pos in self.distance_manager.take_tickets_to_release() {
                if self.distance_manager.has_player_loading_ticket(pos) {
                    let holder = self.holders.map.get(&pos).expect("holder of a player ticket");
                    let future = holder.entity_ticking_chunk_future;
                    self.holders.futures.then(future, Waiter::ReleasePlayerTicket(pos));
                }
            }
            updated
        };
        let promoted = std::mem::take(&mut self.holders.modified);
        self.run_generation_tasks();
        self.process_woken();
        if !result && !promoted {
            return false;
        }
        self.clear_cache();
        true
    }

    fn update_highest_allowed_status(&mut self, node: i64) {
        let Some(holder) = self.holders.map.get_mut(&node) else { return };
        let old_status = holder.highest_allowed_status;
        let new_status = chunk_level::generation_status(holder.ticket_level);
        holder.highest_allowed_status = new_status;
        let dropped = old_status.is_some_and(|old| new_status.is_none_or(|new| new < old));
        if dropped {
            self.fail_and_clear_pending_futures_between(node, new_status, old_status.unwrap());
            let holder = &self.holders.map[&node];
            if holder.task.is_some() {
                let status = self.find_highest_status_with_pending_future(node, new_status);
                self.reschedule_chunk_task(node, status);
            }
        }
    }

    fn fail_and_clear_pending_futures_between(&mut self, node: i64, from_exclusive: Option<ChunkStatus>, to_inclusive: ChunkStatus) {
        let start = from_exclusive.map_or(0, |s| s.index() + 1);
        let holder = self.holders.map.get_mut(&node).unwrap();
        for i in start..=to_inclusive.index() {
            if let Some(future) = holder.futures[i]
                && self.holders.futures.complete(future, false)
            {
                holder.futures[i] = None;
            }
        }
    }

    fn find_highest_status_with_pending_future(&self, node: i64, new_status: Option<ChunkStatus>) -> Option<ChunkStatus> {
        let holder = &self.holders.map[&node];
        let mut highest = new_status?;
        while holder.started_work.is_none_or(|started| highest > started) {
            if holder.futures[highest.index()].is_some() {
                return Some(highest);
            }
            if highest == ChunkStatus::Empty {
                break;
            }
            highest = highest.parent();
        }
        None
    }

    fn update_futures(&mut self, node: i64) {
        let holder = &self.holders.map[&node];
        let old_full_status = chunk_level::full_status(holder.old_ticket_level);
        let new_full_status = chunk_level::full_status(holder.ticket_level);
        let was_accessible = old_full_status.is_or_after(FullChunkStatus::Full);
        let is_accessible = new_full_status.is_or_after(FullChunkStatus::Full);
        self.holders.map.get_mut(&node).unwrap().was_accessible_since_last_save |= is_accessible;
        if !was_accessible && is_accessible {
            let future = self.prepare_accessible_chunk(node);
            self.holders.map.get_mut(&node).unwrap().full_chunk_future = future;
            self.schedule_full_chunk_promotion(node, future, FullChunkStatus::Full);
        }
        if was_accessible && !is_accessible {
            let holder = self.holders.map.get_mut(&node).unwrap();
            self.holders.futures.complete(holder.full_chunk_future, false);
            holder.full_chunk_future = UNLOADED;
        }
        let was_ticking = old_full_status.is_or_after(FullChunkStatus::BlockTicking);
        let is_ticking = new_full_status.is_or_after(FullChunkStatus::BlockTicking);
        if !was_ticking && is_ticking {
            let future = self.prepare_ticking_chunk_future(node);
            self.holders.map.get_mut(&node).unwrap().ticking_chunk_future = future;
            self.schedule_full_chunk_promotion(node, future, FullChunkStatus::BlockTicking);
        }
        if was_ticking && !is_ticking {
            let holder = self.holders.map.get_mut(&node).unwrap();
            self.holders.futures.complete(holder.ticking_chunk_future, false);
            holder.ticking_chunk_future = UNLOADED;
        }
        let was_entity_ticking = old_full_status.is_or_after(FullChunkStatus::EntityTicking);
        let is_entity_ticking = new_full_status.is_or_after(FullChunkStatus::EntityTicking);
        if !was_entity_ticking && is_entity_ticking {
            assert_eq!(self.holders.map[&node].entity_ticking_chunk_future, UNLOADED, "entity ticking future already there");
            let future = self.prepare_entity_ticking_chunk(node);
            self.holders.map.get_mut(&node).unwrap().entity_ticking_chunk_future = future;
            self.schedule_full_chunk_promotion(node, future, FullChunkStatus::EntityTicking);
        }
        if was_entity_ticking && !is_entity_ticking {
            let holder = self.holders.map.get_mut(&node).unwrap();
            self.holders.futures.complete(holder.entity_ticking_chunk_future, false);
            holder.entity_ticking_chunk_future = UNLOADED;
        }
        if !new_full_status.is_or_after(old_full_status) {
            self.holders.map.get_mut(&node).unwrap().pending_full_state_confirmation += 1;
            self.full_status_changes.push_back((ChunkPos::unpack(node), new_full_status));
        }
        let holder = self.holders.map.get_mut(&node).unwrap();
        self.worldgen_queue.resort_chunk_tasks(holder.queue_level, node, holder.ticket_level);
        holder.queue_level = holder.ticket_level;
        holder.old_ticket_level = holder.ticket_level;
    }

    fn schedule_full_chunk_promotion(&mut self, node: i64, future: FutureId, status: FullChunkStatus) {
        let holder = self.holders.map.get_mut(&node).unwrap();
        holder.pending_full_state_confirmation += 1;
        let waiter = Waiter::FullStatus { future, node, holder: holder.id, token: holder.pending_full_state_confirmation, status };
        self.holders.futures.then(future, waiter);
    }

    fn chunk_range_future(&mut self, center: i64, range: i32, status_at: impl Fn(i32) -> Option<ChunkStatus>, then: impl FnOnce(&mut Self) -> RangeThen) -> FutureId {
        let pos = ChunkPos::unpack(center);
        let mut deps = Vec::new();
        for z in -range..=range {
            for x in -range..=range {
                let distance = x.abs().max(z.abs());
                let node = ChunkPos::new(pos.x + x, pos.z + z).pack();
                if !self.holders.map.contains_key(&node) {
                    return UNLOADED;
                }
                let status = status_at(distance).expect("status in range");
                deps.push(self.schedule_chunk_generation_task(node, status));
            }
        }
        let then = then(self);
        let target = match then {
            RangeThen::Complete(f) | RangeThen::Ticking { future: f, .. } => f,
        };
        let range = self.holders.add_range(RangeWait { deps: deps.clone(), remaining: deps.len(), then });
        for dep in deps {
            self.holders.futures.then(dep, Waiter::Range(range));
        }
        target
    }

    fn prepare_accessible_chunk(&mut self, node: i64) -> FutureId {
        self.chunk_range_future(node, 1, chunk_level::status_around_full_chunk, |c| RangeThen::Complete(c.holders.futures.new_pending()))
    }

    fn prepare_ticking_chunk_future(&mut self, node: i64) -> FutureId {
        let pos = ChunkPos::unpack(node);
        self.chunk_range_future(node, 1, |_| Some(ChunkStatus::Full), |c| RangeThen::Ticking { pos, future: c.holders.futures.new_pending() })
    }

    fn prepare_entity_ticking_chunk(&mut self, node: i64) -> FutureId {
        self.chunk_range_future(node, 2, |_| Some(ChunkStatus::Full), |c| RangeThen::Complete(c.holders.futures.new_pending()))
    }

    fn finish_range(&mut self, range: u64) {
        let wait = self.holders.ranges.remove(&range).unwrap();
        let success = wait.deps.iter().all(|&d| self.holders.futures.is_success(d));
        match wait.then {
            RangeThen::Complete(future) => {
                self.holders.futures.complete(future, success);
            }
            RangeThen::Ticking { pos, future } => {
                if success {
                    self.ticking_ready.push(pos);
                }
                self.holders.futures.complete(future, success);
            }
        }
    }

    fn schedule_chunk_generation_task(&mut self, node: i64, status: ChunkStatus) -> FutureId {
        let holder = &self.holders.map[&node];
        if holder.is_status_disallowed(status) {
            return UNLOADED;
        }
        let future = self.get_or_create_future(node, status);
        if self.holders.futures.is_done(future) {
            return future;
        }
        let holder = &self.holders.map[&node];
        if holder.task.is_none_or(|t| status > self.holders.tasks[&t].target_status) {
            self.reschedule_chunk_task(node, Some(status));
        }
        future
    }

    fn get_or_create_future(&mut self, node: i64, status: ChunkStatus) -> FutureId {
        let holder = self.holders.get_mut(node).unwrap();
        if holder.is_status_disallowed(status) {
            return UNLOADED;
        }
        match holder.futures[status.index()] {
            Some(future) => future,
            None => {
                let future = self.holders.futures.new_pending();
                self.holders.get_mut(node).unwrap().futures[status.index()] = Some(future);
                future
            }
        }
    }

    fn reschedule_chunk_task(&mut self, node: i64, status: Option<ChunkStatus>) {
        let new_task = status.map(|s| self.schedule_generation_task(s, ChunkPos::unpack(node)));
        let holder = self.holders.get_mut(node).unwrap();
        let old_task = std::mem::replace(&mut holder.task, new_task);
        if let Some(old) = old_task
            && let Some(task) = self.holders.tasks.get_mut(&old)
        {
            task.marked_for_cancellation = true;
        }
    }

    fn schedule_generation_task(&mut self, target_status: ChunkStatus, pos: ChunkPos) -> u64 {
        let radius = target_status.accumulated_radius_of(ChunkStatus::Empty);
        for x in pos.x - radius..=pos.x + radius {
            for z in pos.z - radius..=pos.z + radius {
                self.holders.get_mut(ChunkPos::new(x, z).pack()).expect("holder around a generating chunk").generation_ref_count += 1;
            }
        }
        let task = ChunkGenerationTask {
            pos,
            target_status,
            scheduled_status: None,
            marked_for_cancellation: false,
            scheduled_layer: Vec::new(),
            radius,
            needs_generation: false,
        };
        let id = self.holders.add_task(task);
        self.holders.pending_generation_tasks.push(id);
        id
    }

    fn run_generation_tasks(&mut self) {
        for task in std::mem::take(&mut self.holders.pending_generation_tasks) {
            self.run_generation_task(task);
        }
    }

    fn run_generation_task(&mut self, task: u64) {
        let Some(t) = self.holders.tasks.get(&task) else { return };
        let node = t.pos.pack();
        let level = self.holders.get(node).map_or(chunk_level::MAX_LEVEL + 1, |h| h.queue_level);
        self.worldgen_queue.submit(task, node, level.min(super::chunk_task_dispatcher::PRIORITY_LEVEL_COUNT - 1));
    }

    fn run_worldgen(&mut self) -> bool {
        let mut any = false;
        while let Some((_, tasks)) = self.worldgen_queue.pop() {
            any = true;
            for task in tasks {
                self.run_until_wait(task);
            }
            self.process_woken();
        }
        any
    }

    fn run_until_wait(&mut self, id: u64) {
        if !self.holders.tasks.contains_key(&id) {
            return;
        }
        loop {
            if let Some(waiting_for) = self.wait_for_scheduled_layer(id) {
                self.holders.futures.then(waiting_for, Waiter::Task(id));
                return;
            }
            let task = &self.holders.tasks[&id];
            if task.marked_for_cancellation || task.scheduled_status == Some(task.target_status) {
                self.release_claim(id);
                return;
            }
            self.schedule_next_layer(id);
        }
    }

    fn wait_for_scheduled_layer(&mut self, id: u64) -> Option<FutureId> {
        let ChunkHolders { tasks, futures, .. } = &mut self.holders;
        let task = tasks.get_mut(&id).unwrap();
        while let Some(&last) = task.scheduled_layer.last() {
            let Some(result) = futures.result(last) else { return Some(last) };
            task.scheduled_layer.pop();
            if !result {
                task.marked_for_cancellation = true;
            }
        }
        None
    }

    fn release_claim(&mut self, id: u64) {
        let task = self.holders.tasks.remove(&id).unwrap();
        let center = task.pos.pack();
        if let Some(holder) = self.holders.get_mut(center)
            && holder.task == Some(id)
        {
            holder.task = None;
        }
        for x in task.pos.x - task.radius..=task.pos.x + task.radius {
            for z in task.pos.z - task.radius..=task.pos.z + task.radius {
                if let Some(holder) = self.holders.get_mut(ChunkPos::new(x, z).pack()) {
                    holder.generation_ref_count -= 1;
                }
            }
        }
    }

    fn schedule_next_layer(&mut self, id: u64) {
        let task = &self.holders.tasks[&id];
        let status = match task.scheduled_status {
            None => ChunkStatus::Empty,
            Some(ChunkStatus::Empty) if !task.needs_generation && !self.can_load_without_generation(id) => {
                self.holders.tasks.get_mut(&id).unwrap().needs_generation = true;
                ChunkStatus::Empty
            }
            Some(status) => status.next(),
        };
        let needs_generation = self.holders.tasks[&id].needs_generation;
        self.schedule_layer(id, status, needs_generation);
        self.holders.tasks.get_mut(&id).unwrap().scheduled_status = Some(status);
    }

    fn can_load_without_generation(&self, id: u64) -> bool {
        let task = &self.holders.tasks[&id];
        if task.target_status == ChunkStatus::Empty {
            return true;
        }
        let persisted = |x: i32, z: i32| self.holders.get(ChunkPos::new(x, z).pack()).and_then(|h| h.persisted_status);
        if !persisted(task.pos.x, task.pos.z).is_some_and(|s| s >= task.target_status) {
            return false;
        }
        let range = task.target_status.dependency_radius();
        for x in task.pos.x - range..=task.pos.x + range {
            for z in task.pos.z - range..=task.pos.z + range {
                let distance = task.pos.chessboard_distance(ChunkPos::new(x, z));
                let required = task.target_status.dependency_at(distance);
                if persisted(x, z).is_none_or(|s| s < required) {
                    return false;
                }
            }
        }
        true
    }

    fn schedule_layer(&mut self, id: u64, status: ChunkStatus, needs_generation: bool) {
        let task = &self.holders.tasks[&id];
        let (pos, radius) = (task.pos, task.target_status.accumulated_radius_of(status));
        for x in pos.x - radius..=pos.x + radius {
            for z in pos.z - radius..=pos.z + radius {
                if self.holders.tasks[&id].marked_for_cancellation || !self.schedule_chunk_in_layer(id, status, needs_generation, ChunkPos::new(x, z)) {
                    return;
                }
            }
        }
    }

    fn schedule_chunk_in_layer(&mut self, id: u64, status: ChunkStatus, needs_generation: bool, pos: ChunkPos) -> bool {
        let persisted = self.holders.get(pos.pack()).unwrap().persisted_status;
        let generate = persisted.is_some_and(|p| status > p);
        if generate && !needs_generation {
            panic!("Can't load chunk, but didn't expect to need to generate: {pos:?}");
        }
        let future = self.apply_step(pos, status, generate);
        match self.holders.futures.result(future) {
            None => {
                self.holders.tasks.get_mut(&id).unwrap().scheduled_layer.push(future);
                true
            }
            Some(true) => true,
            Some(false) => {
                self.holders.tasks.get_mut(&id).unwrap().marked_for_cancellation = true;
                false
            }
        }
    }

    fn apply_step(&mut self, pos: ChunkPos, status: ChunkStatus, generate: bool) -> FutureId {
        let node = pos.pack();
        let holder = self.holders.get_mut(node).unwrap();
        if holder.is_status_disallowed(status) {
            return UNLOADED;
        }
        if !holder.acquire_status_bump(status) {
            return self.get_or_create_future(node, status);
        }
        let holder = holder.id;
        let work = self.holders.futures.new_pending();
        let job = Job { pos, holder, status, work };
        self.start_step(job, generate);
        work
    }

    fn new_token(&mut self, job: Job) -> u64 {
        self.next_token += 1;
        self.jobs.insert(self.next_token, job);
        self.next_token
    }

    fn start_step(&mut self, job: Job, generate: bool) {
        let pos = job.pos;
        match job.status {
            ChunkStatus::Empty => self.schedule_chunk_load(job),
            ChunkStatus::StructureStarts if generate => match (&self.generator, &self.structure_state) {
                (generator, Some(state)) if self.generate_structures => {
                    let (generator, state, dimension, results) = (generator.clone(), state.clone(), self.dimension, self.results.0.clone());
                    let token = self.new_token(job);
                    self.in_flight += 1;
                    chunk_workers::execute(move || {
                        let mut structures = ChunkStructures::default();
                        generator.create_structures(&state, pos, dimension, &mut structures);
                        let _ = results.send(WorkResult::StructureStarts { pos, token, structures });
                    });
                }
                _ => {
                    self.structure_chunks.insert(pos, ChunkStructures::default());
                    self.complete_step(job);
                }
            },
            ChunkStatus::StructureReferences if generate => {
                self.create_references(pos);
                self.complete_step(job);
            }
            ChunkStatus::StructureStarts | ChunkStatus::StructureReferences | ChunkStatus::Biomes | ChunkStatus::Spawn => self.complete_step(job),
            ChunkStatus::Terrain if generate => {
                let beardifier = self.beardifier_for_structures_in_chunk(pos).map(Arc::new);
                let token = self.new_token(job);
                let (generator, dimension, results) = (self.generator.clone(), self.dimension, self.results.0.clone());
                self.in_flight += 1;
                chunk_workers::execute(move || {
                    let chunk = generator.generate_with_beardifier(pos, dimension, beardifier);
                    let _ = results.send(WorkResult::Generated { pos, token, chunk });
                });
            }
            ChunkStatus::Terrain => {
                let holder = self.holders.get_mut(pos.pack()).unwrap();
                let decorated = holder.persisted_status.is_some_and(|status| status >= ChunkStatus::Features);
                if let Some(chunk) = holder.loaded.take() {
                    if decorated {
                        self.chunks.insert(pos, *chunk);
                    } else {
                        self.terrain_chunks.insert(pos, *chunk);
                    }
                }
                self.complete_step(job);
            }
            ChunkStatus::Features => {
                if generate {
                    self.generate_features(pos);
                }
                self.complete_step(job);
            }
            ChunkStatus::InitializeLight => {
                let lighted = self.is_lighted(pos);
                if let Some(chunk) = self.chunks.get_mut(&pos) {
                    chunk.initialize_light_sources();
                }
                let token = self.new_token(job);
                self.add_light_task(TaskType::PreUpdate, LightTask::InitializeLight(pos));
                self.add_light_task(TaskType::PostUpdate, LightTask::InitializeLightDone { pos, lighted, done: token });
            }
            ChunkStatus::Light => {
                let lighted = self.is_lighted(pos);
                if let Some(chunk) = self.chunks.get_mut(&pos) {
                    chunk.light_correct = false;
                }
                let token = self.new_token(job);
                self.add_light_task(TaskType::PreUpdate, LightTask::LightChunk { pos, lighted });
                self.add_light_task(TaskType::PostUpdate, LightTask::LightChunkDone { pos, done: token });
            }
            ChunkStatus::Full => {
                let mut proto_ticks = None;
                if let Some(chunk) = self.chunks.get_mut(&pos) {
                    if let Some(proto) = chunk.proto.take() {
                        let proto = *proto;
                        proto_ticks = Some((LevelChunkTicks::with_pending(proto.block_ticks.scheduled_ticks()), LevelChunkTicks::with_pending(proto.fluid_ticks.scheduled_ticks())));
                    }
                    chunk.loaded = true;
                    chunk.register_all_block_entities_after_level_load(&mut self.block_entity_tickers);
                }
                let holder = self.holders.get_mut(pos.pack()).unwrap();
                let (block_ticks, fluid_ticks) = match holder.saved_ticks.take() {
                    Some(ticks) => *ticks,
                    None => proto_ticks.unwrap_or_default(),
                };
                self.block_ticks.register_container(pos, block_ticks);
                self.fluid_ticks.register_container(pos, fluid_ticks);
                self.complete_step(job);
            }
        }
    }

    fn chunk_structures(&self, pos: ChunkPos) -> Option<&ChunkStructures> {
        if let Some(structures) = self.structure_chunks.get(&pos) {
            return Some(structures);
        }
        if let Some(chunk) = self.terrain_chunks.get(&pos).or_else(|| self.chunks.get(&pos)) {
            return Some(&chunk.structures);
        }
        self.holders.get(pos.pack()).and_then(|holder| holder.loaded.as_ref()).map(|chunk| &chunk.structures)
    }

    fn chunk_structures_mut(&mut self, pos: ChunkPos) -> Option<&mut ChunkStructures> {
        if self.structure_chunks.contains_key(&pos) {
            return self.structure_chunks.get_mut(&pos);
        }
        if self.terrain_chunks.contains_key(&pos) {
            return self.terrain_chunks.get_mut(&pos).map(|chunk| &mut chunk.structures);
        }
        if self.chunks.contains_key(&pos) {
            return self.chunks.get_mut(&pos).map(|chunk| &mut chunk.structures);
        }
        self.holders.get_mut(pos.pack()).and_then(|holder| holder.loaded.as_mut()).map(|chunk| &mut chunk.structures)
    }

    fn create_references(&mut self, center: ChunkPos) {
        let (target_block_x, target_block_z) = (center.min_block_x(), center.min_block_z());
        let mut references = Vec::new();
        for source_x in center.x - 8..=center.x + 8 {
            for source_z in center.z - 8..=center.z + 8 {
                let source_chunk_key = ChunkPos::new(source_x, source_z).pack();
                let Some(structures) = self.chunk_structures(ChunkPos::new(source_x, source_z)) else { continue };
                for (&structure, start) in &structures.starts {
                    if start.is_valid() && start.get_bounding_box().intersects_xz(target_block_x, target_block_z, target_block_x + 15, target_block_z + 15) {
                        references.push((structure, source_chunk_key));
                    }
                }
            }
        }
        if let Some(structures) = self.chunk_structures_mut(center) {
            for (structure, source_chunk_key) in references {
                structures.add_reference_for_structure(structure, source_chunk_key);
            }
        }
    }

    pub fn starts_for_structure(&self, pos: ChunkPos, structure: StructureHolder) -> Vec<Arc<StructureStart>> {
        let Some(references) = self.chunk_structures(pos).and_then(|structures| structures.get_references_for_structure(structure)) else { return Vec::new() };
        references
            .iter()
            .filter_map(|key| self.chunk_structures(ChunkPos::unpack(key))?.get_start_for_structure(structure).filter(|start| start.is_valid()).cloned())
            .collect()
    }

    fn beardifier_for_structures_in_chunk(&self, pos: ChunkPos) -> Option<Beardifier> {
        let references = self.chunk_structures(pos)?;
        let structure_starts: Vec<Arc<StructureStart>> = references
            .references
            .keys()
            .filter(|structure| structure.value().terrain_adaptation() != TerrainAdjustment::None)
            .flat_map(|&structure| self.starts_for_structure(pos, structure))
            .collect();
        Beardifier::for_structures_in_chunk(&structure_starts, pos)
    }

    fn generate_features(&mut self, center: ChunkPos) {
        self.apply_biome_decoration(center);
        if let Some(chunk) = self.terrain_chunks.remove(&center) {
            self.chunks.insert(center, chunk);
        }
    }

    fn apply_biome_decoration(&mut self, center: ChunkPos) {
        let dimension = self.dimension;
        let generator = self.generator.clone();
        let region_random_state = self.region_random_state.clone();
        let mut structure_starts = Vec::new();
        if self.generate_structures {
            structure_starts.resize_with(crisp_data::STRUCTURES.len(), Vec::new);
            let referenced: Vec<StructureHolder> = self.chunk_structures(center).map(|structures| structures.references.keys().copied().collect()).unwrap_or_default();
            for structure in referenced {
                structure_starts[structure.0 as usize] = self.starts_for_structure(center, structure);
            }
        }
        let mut taken: Vec<(ChunkPos, Option<LevelChunk>, bool)> = Vec::with_capacity(9);
        let mut writable = [false; 9];
        let mut lit = [false; 9];
        for (i, (dx, dz)) in (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz))).enumerate() {
            let pos = ChunkPos::new(center.x + dx, center.z + dz);
            let (chunk, terrain) = match self.terrain_chunks.remove(&pos) {
                Some(chunk) => (Some(chunk), true),
                None => (self.chunks.remove(&pos), false),
            };
            writable[i] = chunk.as_ref().is_some_and(|c| c.proto.is_some());
            lit[i] = self.holders.get(pos.pack()).is_some_and(|h| h.status_done(&self.holders.futures, ChunkStatus::InitializeLight));
            taken.push((pos, chunk, terrain));
        }
        let light_tasks = {
            let mut iter = taken.iter_mut();
            let cache: [Option<&mut LevelChunk>; 9] = std::array::from_fn(|_| iter.next().and_then(|(_, c, _)| c.as_mut()));
            let generation = WorldGenerationContext {
                min_gen_y: dimension.min_y.max(generator.min_y()),
                gen_depth: (dimension.height as i32).min(generator.gen_depth()),
                sea_level: generator.sea_level(),
            };
            let random_state = match &generator {
                ChunkGenerator::Noise(noise) => &noise.state,
                ChunkGenerator::Flat(_) => region_random_state.as_deref().expect("a flat random state"),
            };
            let mut region = WorldGenRegion::new(center, cache, writable, lit, &generator, random_state, dimension.min_y, dimension.height as i32, generation, &self.light_engine.engine, self.difficulty);
            generator.apply_biome_decoration(&mut region, &structure_starts);
            std::mem::take(&mut region.light_tasks)
        };
        for (i, (pos, chunk, terrain)) in taken.into_iter().enumerate() {
            if let Some(mut chunk) = chunk {
                chunk.unsaved |= writable[i];
                if terrain {
                    self.terrain_chunks.insert(pos, chunk);
                } else {
                    self.chunks.insert(pos, chunk);
                }
            }
        }
        for task in light_tasks {
            match task {
                RegionLightTask::UpdateSectionStatus(pos, empty) => {
                    self.add_light_task(TaskType::PreUpdate, LightTask::UpdateSectionStatus(section::of(pos.x >> 4, pos.y >> 4, pos.z >> 4), empty));
                }
                RegionLightTask::CheckBlock(pos) => self.add_light_task(TaskType::PreUpdate, LightTask::CheckBlock(pos)),
            }
        }
    }

    fn is_lighted(&self, pos: ChunkPos) -> bool {
        let persisted = self.holders.get(pos.pack()).and_then(|h| h.persisted_status);
        persisted.is_some_and(|s| s >= ChunkStatus::Light) && self.chunks.get(&pos).is_some_and(|c| c.light_correct)
    }

    fn schedule_chunk_load(&mut self, job: Job) {
        let pos = job.pos;
        if self.io.is_none() {
            self.apply_loaded(job, Ok(None));
            return;
        }
        let token = self.new_token(job);
        let (dimension, seed, results) = (self.dimension, self.seed, self.results.0.clone());
        self.in_flight += 1;
        self.io.as_ref().unwrap().read(pos, move |read| {
            chunk_workers::execute(move || {
                let result = read.and_then(|nbt| nbt.map(|nbt| serialization::read(&nbt, dimension, seed)).transpose());
                let _ = results.send(WorkResult::Loaded { pos, token, result });
            });
        });
    }

    fn apply_loaded(&mut self, job: Job, result: io::Result<Option<serialization::ReadChunk>>) {
        let pos = job.pos;
        let read = match result {
            Ok(read) => read,
            Err(_) => {
                error!("Failed to load chunk {},{}", pos.x, pos.z);
                None
            }
        };
        let persisted = match read {
            Some(serialization::ReadChunk { chunk, status, block_ticks, fluid_ticks, light }) => {
                let mut loaded_any_light = false;
                for SectionLight { y, block, sky } in light {
                    let sky = sky.filter(|_| self.dimension.has_skylight);
                    if block.is_none() && sky.is_none() {
                        continue;
                    }
                    if !loaded_any_light {
                        self.add_light_task(TaskType::PreUpdate, LightTask::RetainData(pos, true));
                        loaded_any_light = true;
                    }
                    let section_node = section::of(pos.x, y, pos.z);
                    if block.is_some() {
                        self.add_light_task(TaskType::PreUpdate, LightTask::QueueSectionData(LightLayer::Block, section_node, block));
                    }
                    if sky.is_some() {
                        self.add_light_task(TaskType::PreUpdate, LightTask::QueueSectionData(LightLayer::Sky, section_node, sky));
                    }
                }
                if status < ChunkStatus::Terrain {
                    self.structure_chunks.insert(pos, chunk.structures);
                    self.loaded_structure_chunks.insert(pos);
                } else {
                    let holder = self.holders.get_mut(pos.pack()).unwrap();
                    holder.loaded = Some(Box::new(chunk));
                    if status == ChunkStatus::Full {
                        holder.saved_ticks = Some(Box::new((block_ticks, fluid_ticks)));
                    }
                }
                status
            }
            None => ChunkStatus::Empty,
        };
        self.holders.get_mut(pos.pack()).unwrap().persisted_status = Some(persisted);
        self.complete_step(job);
    }

    fn apply_work_result(&mut self, result: WorkResult) {
        let (token, pos) = match &result {
            WorkResult::Loaded { token, pos, .. } | WorkResult::Generated { token, pos, .. } | WorkResult::StructureStarts { token, pos, .. } => (*token, *pos),
            WorkResult::Wake => return,
        };
        self.in_flight -= 1;
        let job = self.jobs.remove(&token).expect("job of a result");
        if self.holders.get(pos.pack()).is_none_or(|h| h.id != job.holder) {
            self.holders.futures.complete(job.work, false);
            return;
        }
        match result {
            WorkResult::Loaded { result, .. } => self.apply_loaded(job, result),
            WorkResult::Generated { mut chunk, .. } => {
                chunk.unsaved = true;
                chunk.structures = self.structure_chunks.remove(&pos).unwrap_or_default();
                self.loaded_structure_chunks.remove(&pos);
                self.terrain_chunks.insert(pos, chunk);
                self.complete_step(job);
            }
            WorkResult::StructureStarts { structures, .. } => {
                self.structure_chunks.insert(pos, structures);
                self.complete_step(job);
            }
            WorkResult::Wake => {}
        }
        self.process_woken();
    }

    fn complete_step(&mut self, job: Job) {
        let node = job.pos.pack();
        let Some(holder) = self.holders.get_mut(node).filter(|h| h.id == job.holder) else {
            self.holders.futures.complete(job.work, false);
            return;
        };
        if job.status != ChunkStatus::Empty && holder.persisted_status.is_some_and(|p| p < job.status) {
            holder.persisted_status = Some(job.status);
        }
        let index = job.status.index();
        let existing = holder.futures[index];
        let completed = match existing {
            Some(future) if self.holders.futures.complete(future, true) => true,
            Some(future) if self.holders.futures.is_success(future) => {
                panic!("Trying to complete a future but found it to be completed successfully already");
            }
            _ => false,
        };
        if !completed {
            let done = self.holders.futures.new_done(true);
            self.holders.get_mut(node).unwrap().futures[index] = Some(done);
        }
        self.holders.futures.complete(job.work, true);
    }

    fn add_light_task(&mut self, ty: TaskType, task: LightTask) {
        self.light_engine.add_task(&mut self.chunks, ty, task, &mut self.light_changes, &mut self.light_done);
        self.finish_light_steps();
        self.on_light_updates();
    }

    fn run_light_batch(&mut self) {
        self.light_engine.run_update(&mut self.chunks, &mut self.light_changes, &mut self.light_done);
        self.finish_light_steps();
        self.on_light_updates();
        self.process_woken();
    }

    fn finish_light_steps(&mut self) {
        for token in std::mem::take(&mut self.light_done) {
            let job = self.jobs.remove(&token).expect("light step");
            self.complete_step(job);
        }
    }

    pub fn run_light_updates(&mut self) {
        while self.light_engine.has_work() {
            self.run_light_batch();
        }
        self.on_light_updates();
    }

    fn on_light_updates(&mut self) {
        for (layer, section_node) in self.light_changes.drain(..) {
            let pos = ChunkPos::new(section::x(section_node), section::z(section_node));
            let Some(holder) = self.holders.map.get(&pos.pack()) else { continue };
            if !holder.status_done(&self.holders.futures, ChunkStatus::InitializeLight) || holder.is_status_disallowed(ChunkStatus::InitializeLight) {
                continue;
            }
            if let Some(chunk) = self.chunks.get_mut(&pos) {
                chunk.unsaved = true;
            }
            if holder.is_ticking(&self.holders.futures) {
                self.ticking_light_changes.push((layer, section_node));
            }
        }
    }

    pub fn take_light_changes(&mut self) -> Vec<(LightLayer, i64)> {
        self.on_light_updates();
        std::mem::take(&mut self.ticking_light_changes)
    }

    pub fn set_block_state(&mut self, pos: BlockPos, state: BlockState) -> Option<BlockState> {
        let chunk = self.chunk(pos.chunk());
        let index = ((pos.y - chunk.min_y()) >> 4) as usize;
        let was_empty = chunk.sections[index].has_only_air();
        let old = chunk.set_block_state(pos, state)?;
        chunk.unsaved = true;
        let is_empty = chunk.sections[index].has_only_air();
        let light_changed = lighting::has_different_light_properties(old, state);
        if light_changed {
            chunk.update_sky_light_sources(pos);
        }
        if was_empty != is_empty {
            let section_node = section::of(pos.x >> 4, pos.y >> 4, pos.z >> 4);
            self.add_light_task(TaskType::PreUpdate, LightTask::UpdateSectionStatus(section_node, is_empty));
        }
        if light_changed {
            self.add_light_task(TaskType::PreUpdate, LightTask::CheckBlock(pos));
        }
        Some(old)
    }

    pub fn block_entity(&mut self, pos: BlockPos, creation: EntityCreationType) -> Option<&mut BlockEntity> {
        let chunk_pos = pos.chunk();
        self.chunk(chunk_pos);
        let ChunkCache { chunks, block_entity_tickers, .. } = self;
        chunks.get_mut(&chunk_pos).unwrap().get_block_entity(pos, creation, block_entity_tickers)
    }

    pub fn block_entity_ref(&self, pos: BlockPos) -> Option<&BlockEntity> {
        self.chunks.get(&pos.chunk())?.block_entities.map.get(&pos).filter(|e| !e.is_removed())
    }

    pub fn remove_block_entity(&mut self, pos: BlockPos) -> Option<BlockEntity> {
        self.chunk(pos.chunk()).remove_block_entity(pos)
    }

    pub fn add_and_register_block_entity(&mut self, entity: BlockEntity) {
        let chunk_pos = entity.pos().chunk();
        self.chunk(chunk_pos);
        let ChunkCache { chunks, block_entity_tickers, .. } = self;
        chunks.get_mut(&chunk_pos).unwrap().add_and_register_block_entity(entity, block_entity_tickers);
    }

    pub fn update_block_entity_ticker(&mut self, pos: BlockPos) {
        let chunk_pos = pos.chunk();
        self.chunk(chunk_pos);
        let ChunkCache { chunks, block_entity_tickers, .. } = self;
        chunks.get_mut(&chunk_pos).unwrap().update_block_entity_ticker(pos, block_entity_tickers);
    }

    pub fn bound_ticker(&self, id: u64, pos: BlockPos) -> Option<crate::world::level::block::entity::BlockEntityTicker> {
        self.chunks.get(&pos.chunk())?.bound_ticker(id, pos)
    }

    pub fn write_chunk_with_light(&mut self, pos: ChunkPos, w: &mut Writer) {
        if !self.chunks.contains_key(&pos) {
            self.chunk(pos);
        }
        let chunk = &self.chunks[&pos];
        w.i32(pos.x);
        w.i32(pos.z);
        chunk.write_data(w);
        lighting::write_light_data(w, pos, &self.light_engine.engine, None, None);
    }

    fn process_unloads(&mut self, have_time: bool) {
        let mut dropped = Vec::new();
        self.holders.to_drop.retain(|pos| {
            dropped.push(pos);
            false
        });
        for pos in dropped {
            if let Some(holder) = self.holders.map.remove(&pos) {
                self.holders.pending_unloads.insert(pos, holder);
                self.holders.modified = true;
                self.waiting_unloads.insert(pos);
            }
        }
        let ready: Vec<i64> = self
            .waiting_unloads
            .iter()
            .copied()
            .filter(|pos| self.holders.pending_unloads.get(pos).is_none_or(|h| h.is_ready_for_saving(&self.holders.futures)))
            .collect();
        for pos in ready {
            self.waiting_unloads.shift_remove(&pos);
            self.unload_queue.push_back(pos);
        }
        let mut forced = self.unload_queue.len().saturating_sub(2000);
        while forced > 0 || have_time || self.tick_deadline.is_none_or(|deadline| Instant::now() < deadline) {
            let Some(pos) = self.unload_queue.pop_front() else { break };
            forced = forced.saturating_sub(1);
            self.unload(pos);
        }
    }

    fn unload(&mut self, pos: i64) {
        let Some(holder) = self.holders.pending_unloads.get(&pos) else { return };
        if !holder.is_ready_for_saving(&self.holders.futures) {
            self.waiting_unloads.insert(pos);
            return;
        }
        let holder = self.holders.pending_unloads.shift_remove(&pos).unwrap();
        let chunk_pos = holder.pos;
        let is_full = holder.status_done(&self.holders.futures, ChunkStatus::Full);
        let terrain_chunk = self.terrain_chunks.remove(&chunk_pos);
        let structures = self.structure_chunks.remove(&chunk_pos);
        let loaded_structures = self.loaded_structure_chunks.remove(&chunk_pos);
        let mut chunk = self.chunks.remove(&chunk_pos);
        if let Some(chunk) = &mut chunk {
            chunk.loaded = false;
        }
        let had_chunk = chunk.is_some() || terrain_chunk.is_some() || holder.loaded.is_some() || holder.persisted_status.is_some();
        if is_full && let Some(mut chunk) = chunk {
            if self.io.is_some() && chunk.unsaved {
                let block_entities = chunk.block_entities_for_saving(&mut self.block_entity_tickers);
                let light = self.light_to_save(chunk_pos);
                let block_ticks = self.block_ticks.pack(chunk_pos, self.game_time);
                let fluid_ticks = self.fluid_ticks.pack(chunk_pos, self.game_time);
                let save = ChunkToSave { chunk, status: ChunkStatus::Full, block_entities, game_time: self.game_time, block_ticks, fluid_ticks, light };
                self.io.as_ref().unwrap().write_chunk(save);
            }
            self.block_ticks.remove_container(chunk_pos);
            self.fluid_ticks.remove_container(chunk_pos);
        } else if let Some(status) = holder.persisted_status.filter(|&status| status != ChunkStatus::Empty && status != ChunkStatus::Full) {
            let proto_chunk = if status < ChunkStatus::Terrain {
                structures.map(|structures| self.create_proto_chunk(chunk_pos, status, structures, loaded_structures))
            } else {
                terrain_chunk.or(chunk).or(holder.loaded.map(|chunk| *chunk))
            };
            if self.io.is_some()
                && let Some(mut proto_chunk) = proto_chunk
            {
                let block_entities = proto_chunk.block_entities_for_saving(&mut self.block_entity_tickers);
                let light = self.light_to_save(chunk_pos);
                let save = ChunkToSave { chunk: proto_chunk, status, block_entities, game_time: self.game_time, block_ticks: Vec::new(), fluid_ticks: Vec::new(), light };
                self.io.as_ref().unwrap().write_chunk(save);
            }
        }
        if had_chunk {
            self.add_light_task(TaskType::PreUpdate, LightTask::UpdateChunkStatus(chunk_pos));
        }
    }

    fn create_proto_chunk(&self, pos: ChunkPos, status: ChunkStatus, structures: ChunkStructures, loaded: bool) -> LevelChunk {
        let mut chunk = LevelChunk::empty(pos, self.dimension, Biome::by_name("minecraft:plains").expect("plains"));
        if status >= ChunkStatus::Biomes {
            self.generator.fill_biomes_from_noise(&mut chunk, self.dimension);
        }
        chunk.proto = Some(Box::default());
        if loaded {
            for kind in HeightmapType::WORLDGEN {
                chunk.get_height(kind, pos.min_block_x(), pos.min_block_z());
            }
        }
        chunk.structures = structures;
        chunk
    }

    fn light_to_save(&self, pos: ChunkPos) -> Vec<SectionLight> {
        let engine = &self.light_engine.engine;
        let layer = |layer, y| engine.data_layer_data(layer, section::of(pos.x, y, pos.z)).filter(|l| !l.is_empty()).map(|l| l.copy());
        (engine.min_light_section()..engine.max_light_section())
            .map(|y| SectionLight { y, block: layer(LightLayer::Block, y), sky: layer(LightLayer::Sky, y) })
            .filter(|l| l.block.is_some() || l.sky.is_some())
            .collect()
    }

    fn save_chunk(&mut self, pos: ChunkPos) -> bool {
        if self.io.is_none() {
            return false;
        }
        let Some(chunk) = self.chunks.get_mut(&pos).filter(|c| c.unsaved) else { return false };
        let block_entities = chunk.block_entities_for_saving(&mut self.block_entity_tickers);
        let light = self.light_to_save(pos);
        let block_ticks = self.block_ticks.pack(pos, self.game_time);
        let fluid_ticks = self.fluid_ticks.pack(pos, self.game_time);
        let nbt = serialization::write(&self.chunks[&pos], ChunkStatus::Full, block_entities, self.game_time, &block_ticks, &fluid_ticks, &light);
        self.io.as_ref().unwrap().write(pos, nbt);
        self.chunks.get_mut(&pos).unwrap().unsaved = false;
        true
    }

    pub fn save(&mut self, game_time: i64, flush: bool) -> io::Result<usize> {
        if self.io.is_none() {
            return Ok(0);
        }
        self.game_time = game_time;
        if flush {
            self.run_distance_manager_updates();
        }
        let mut full: Vec<ChunkPos> = self
            .holders
            .map
            .values()
            .chain(self.holders.pending_unloads.values())
            .filter(|h| h.status_done(&self.holders.futures, ChunkStatus::Full))
            .map(|h| h.pos)
            .collect();
        full.sort();
        let saved = full.into_iter().filter(|&pos| self.save_chunk(pos)).count();
        if flush {
            self.process_unloads(true);
        }
        self.io.as_ref().unwrap().flush()?;
        Ok(saved)
    }
}

fn is_position_ticking(distance_manager: &DistanceManager, holders: &ChunkHolders, pos: ChunkPos) -> bool {
    let key = pos.pack();
    distance_manager.in_block_ticking_range(key) && holders.map.get(&key).is_some_and(|h| h.is_ticking(&holders.futures))
}

impl BlockGetter for ChunkCache {
    fn block_state(&mut self, pos: BlockPos) -> BlockState {
        if self.is_outside_build_height(pos.y) {
            return blocks::VOID_AIR.default_state();
        }
        self.chunk(pos.chunk()).block_state(pos)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crisp_core::{BlockPos, ChunkPos, Vec3};
    use crisp_data::{DimensionType, blocks};

    use super::ChunkCache;
    use crate::network::configuration::ClientInfo;
    use crate::network::connection::Outbound;
    use crate::network::login::Profile;
    use crate::server::OVERWORLD;
    use crate::server::level::chunk_map::ChunkTrackingView;
    use crate::server::level::server_player::ServerPlayer;
    use crate::server::level::{ServerLevel, testing};
    use crate::world::level::BlockGetter;
    use crate::world::level::chunk::FlatLevelSource;
    use crate::world::level::chunk::storage::RegionStorage;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn overworld() -> &'static DimensionType {
        &crisp_data::DIMENSION_TYPES[0]
    }

    #[test]
    fn noise_chunks_are_decorated_with_ores() {
        let generator = crate::world::level::levelgen::NoiseBasedChunkGenerator::overworld(12345);
        let mut cache = ChunkCache::new(overworld(), crate::world::level::chunk::ChunkGenerator::Noise(std::sync::Arc::new(generator)), None);
        let mut ores = 0;
        for y in -64..40 {
            for z in 0..16 {
                for x in 0..16 {
                    if cache.block_state(BlockPos::new(x, y, z)).block().name().ends_with("_ore") {
                        ores += 1;
                    }
                }
            }
        }
        assert!(ores > 0);
        for cx in -4..4 {
            for cz in -4..4 {
                cache.block_state(BlockPos::new(cx * 16, 0, cz * 16));
            }
        }
        cache.run_light_updates();
    }

    #[test]
    fn decorating_next_to_a_saved_chunk_finishes() {
        let dir = std::env::temp_dir().join(format!("crisp-decorate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let generator = crate::world::level::levelgen::NoiseBasedChunkGenerator::overworld(12345);
        let chunks = ChunkCache::new(overworld(), crate::world::level::chunk::ChunkGenerator::Noise(std::sync::Arc::new(generator)), Some(RegionStorage::new(dir.clone(), false)));
        let mut level = ServerLevel::new(OVERWORLD, 0, chunks, None, 10, 10);
        let pos = BlockPos::new(100, 0, -40);
        level.block_state(pos);
        level.tick();
        level.tick();
        assert!(!level.chunks.has_chunk(pos.chunk()));
        level.block_state(BlockPos::new(84, 0, -56));
        assert!(level.chunks.has_chunk(ChunkPos::new(5, -4)));
        crate::server::level::testing::player(&mut level, crisp_core::Vec3::new(100.5, 80.0, -39.5), 0.0, 0.0);
        for _ in 0..40 {
            level.tick();
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_chunk_needed_now_loads_on_the_spot() {
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), None);
        let pos = ChunkPos::new(5, -3);
        assert!(!cache.has_chunk(pos));
        assert_eq!(cache.block_state(BlockPos::new(80, -61, -40)), blocks::GRASS_BLOCK.default_state());
        assert!(cache.has_chunk(pos));
        assert!(cache.loaded_chunk(pos).is_some());
        assert_eq!(cache.ticket_level(pos), Some(33));
        assert_eq!(cache.ticket_level(ChunkPos::new(6, -3)), Some(34));
        assert!(!cache.has_chunk(ChunkPos::new(6, -3)));
        assert!(cache.chunk_if_present(ChunkPos::new(6, -3)).is_some());
        assert!(cache.chunk_if_present(ChunkPos::new(7, -3)).is_none());
        assert_eq!(cache.ticket_level(ChunkPos::new(16, -3)), Some(44));
        assert_eq!(cache.ticket_level(ChunkPos::new(17, -3)), None);
    }

    #[test]
    fn expired_chunks_are_saved_unloaded_and_read_back() {
        let dir = std::env::temp_dir().join(format!("crisp-unload-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let chunks = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(RegionStorage::new(dir.clone(), false)));
        let mut level = ServerLevel::new(OVERWORLD, 0, chunks, None, 10, 10);
        let pos = BlockPos::new(100, -60, 100);
        level.set_block(pos, blocks::STONE.default_state(), UPDATE_ALL);
        assert!(level.chunks.has_chunk(pos.chunk()));
        level.tick();
        assert!(level.chunks.has_chunk(pos.chunk()));
        level.tick();
        assert!(!level.chunks.has_chunk(pos.chunk()));
        assert_eq!(level.chunks.loaded_chunks_count(), 0);
        assert!(level.chunks.chunk_if_present(pos.chunk()).is_none());
        assert_eq!(level.block_state(pos), blocks::STONE.default_state());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn chunks_reach_a_joining_player_over_time() {
        let mut level = testing::level();
        let profile = Profile { uuid: 7, name: "Joiner".into(), properties: Vec::new() };
        let mut player = ServerPlayer::new(profile, ClientInfo::default(), Outbound::detached(), BlockPos::ZERO);
        player.entity.set_pos(Vec3::new(8.5, -60.0, 8.5));
        player.requested_view_distance = 10;
        let id = player.entity.id;
        level.add_player(player);
        assert!(level.chunks.chunk_if_present(ChunkPos::new(0, 0)).is_none());
        let view = level.player(id).chunk_tracking_view.unwrap();
        let mut in_view = Vec::new();
        ChunkTrackingView::difference(None, Some(view), |p| in_view.push(p), |_| {});
        let mut ready: Vec<ChunkPos> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(120);
        while ready.len() < in_view.len() {
            assert!(Instant::now() < deadline, "only {} chunks ready", ready.len());
            if !level.poll_chunk_task() {
                level.wait_for_chunk_work(Duration::from_millis(5));
            }
            for &pos in &in_view {
                if level.player(id).chunk_sender.is_pending(pos) && !ready.contains(&pos) {
                    ready.push(pos);
                }
            }
        }
        let own = ready.iter().position(|&p| p == ChunkPos::new(0, 0)).unwrap();
        let first_far = ready.iter().position(|p| p.chessboard_distance(ChunkPos::new(0, 0)) >= 6).unwrap();
        assert!(own < first_far, "own chunk {own}, first far chunk {first_far}");
        assert!(in_view.iter().all(|&p| level.chunks.is_ticking(p)));
        assert!(!level.player(id).chunk_sender.is_pending(ChunkPos::new(view.view_distance + 3, 0)));
    }

    #[test]
    fn moving_players_move_the_ticking_chunks() {
        let mut level = testing::level();
        level.set_simulation_distance(3);
        let id = testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
        let ticking = |level: &ServerLevel| level.chunks.block_ticking_chunks();
        assert_eq!(ticking(&level).len(), 49);
        assert!(level.in_entity_ticking_range(ChunkPos::new(-3, 3)));
        level.player_mut(id).entity.set_pos(Vec3::new(5.0 * 16.0 + 8.5, -60.0, 8.5));
        level.move_player(id);
        level.finish_chunk_work();
        level.tick();
        assert!(!level.in_entity_ticking_range(ChunkPos::new(-3, 3)));
        assert!(level.in_entity_ticking_range(ChunkPos::new(8, 3)));
        let now = ticking(&level);
        assert_eq!(now.len(), 49);
        assert!(now.iter().all(|p| (2..=8).contains(&p.x) && (-3..=3).contains(&p.z)));
        let order: Vec<ChunkPos> = level.chunks.distance_manager.entity_ticking_chunks().into_iter().map(ChunkPos::unpack).collect();
        assert_eq!(now, order);
    }
}
