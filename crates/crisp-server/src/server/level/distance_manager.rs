use std::collections::{HashMap, VecDeque};

use crisp_core::fastutil::{Long2ByteOpenHashMap, LongOpenHashSet};
use crisp_core::ChunkPos;

use super::chunk_holder::ChunkHolders;
use super::chunk_level::{self, FullChunkStatus};
use super::chunk_task_dispatcher::{MainThreadTask, ThrottlingChunkTaskDispatcher};
use super::chunk_tracker::{ChunkTracker, TrackerLevels};
use super::ticket::{Ticket, TicketStorage, TicketType, TicketUpdate};
use crate::world::entity::EntityId;

const PLAYER_TICKET_LEVEL: i32 = chunk_level::ENTITY_TICKING_LEVEL;
const SIMULATION_MAX_LEVEL: i32 = 33;
pub const LOADING_MAX_LEVEL: i32 = chunk_level::MAX_LEVEL + 1;
const INSCRIBED_SQUARE_SPAWN_DISTANCE_CHUNK: i32 = 5;

struct FixedPlayerDistanceChunkTracker {
    tracker: ChunkTracker,
    chunks: Long2ByteOpenHashMap,
    max_distance: i32,
}

impl FixedPlayerDistanceChunkTracker {
    fn new(max_distance: i32) -> Self {
        FixedPlayerDistanceChunkTracker {
            tracker: ChunkTracker::new(max_distance + 2),
            chunks: Long2ByteOpenHashMap::new((max_distance + 2) as i8),
            max_distance,
        }
    }
}

struct FixedLevels<'a> {
    chunks: &'a mut Long2ByteOpenHashMap,
    max_distance: i32,
    players_per_chunk: &'a HashMap<i64, Vec<EntityId>>,
    to_update: Option<&'a mut LongOpenHashSet>,
}

impl TrackerLevels for FixedLevels<'_> {
    fn level(&self, node: i64) -> i32 {
        self.chunks.get(node) as i32
    }

    fn set_level(&mut self, node: i64, level: i32) {
        if level > self.max_distance {
            self.chunks.remove(node);
        } else {
            self.chunks.put(node, level as i8);
        }
        if let Some(to_update) = self.to_update.as_deref_mut() {
            to_update.add(node);
        }
    }

    fn level_from_source(&self, node: i64) -> i32 {
        if self.players_per_chunk.get(&node).is_some_and(|p| !p.is_empty()) { 0 } else { i32::MAX }
    }
}

struct SimulationLevels<'a> {
    chunks: &'a mut Long2ByteOpenHashMap,
    tickets: &'a TicketStorage,
}

impl TrackerLevels for SimulationLevels<'_> {
    fn level(&self, node: i64) -> i32 {
        self.chunks.get(node) as i32
    }

    fn set_level(&mut self, node: i64, level: i32) {
        if level >= SIMULATION_MAX_LEVEL {
            self.chunks.remove(node);
        } else {
            self.chunks.put(node, level as i8);
        }
    }

    fn level_from_source(&self, node: i64) -> i32 {
        self.tickets.ticket_level_at(node, true)
    }
}

struct LoadingLevels<'a> {
    holders: &'a mut ChunkHolders,
    tickets: &'a TicketStorage,
}

impl TrackerLevels for LoadingLevels<'_> {
    fn level(&self, node: i64) -> i32 {
        if !self.holders.to_drop.contains(node)
            && let Some(holder) = self.holders.map.get(&node)
        {
            return holder.ticket_level;
        }
        LOADING_MAX_LEVEL
    }

    fn set_level(&mut self, node: i64, level: i32) {
        let old_level = self.holders.map.get(&node).map_or(LOADING_MAX_LEVEL, |h| h.ticket_level);
        if old_level != level && self.holders.update_chunk_scheduling(node, level, old_level) {
            self.holders.chunks_to_update_futures.insert(node);
        }
    }

    fn level_from_source(&self, node: i64) -> i32 {
        self.tickets.ticket_level_at(node, false)
    }
}

pub struct DistanceManager {
    players_per_chunk: HashMap<i64, Vec<EntityId>>,
    natural_spawn_chunk_counter: FixedPlayerDistanceChunkTracker,
    player_ticket_manager: FixedPlayerDistanceChunkTracker,
    view_distance: i32,
    queue_levels: HashMap<i64, i32>,
    to_update: LongOpenHashSet,
    simulation_tracker: ChunkTracker,
    simulation_chunks: Long2ByteOpenHashMap,
    loading_tracker: ChunkTracker,
    pub ticket_storage: TicketStorage,
    ticket_dispatcher: ThrottlingChunkTaskDispatcher,
    tickets_to_release: LongOpenHashSet,
    pub main_thread_tasks: VecDeque<MainThreadTask>,
    simulation_distance: i32,
}

impl Default for DistanceManager {
    fn default() -> Self {
        DistanceManager {
            players_per_chunk: HashMap::new(),
            natural_spawn_chunk_counter: FixedPlayerDistanceChunkTracker::new(8),
            player_ticket_manager: FixedPlayerDistanceChunkTracker::new(32),
            view_distance: 0,
            queue_levels: HashMap::new(),
            to_update: LongOpenHashSet::new(),
            simulation_tracker: ChunkTracker::new(SIMULATION_MAX_LEVEL + 1),
            simulation_chunks: Long2ByteOpenHashMap::new(SIMULATION_MAX_LEVEL as i8),
            loading_tracker: ChunkTracker::new(LOADING_MAX_LEVEL + 1),
            ticket_storage: TicketStorage::default(),
            ticket_dispatcher: ThrottlingChunkTaskDispatcher::new(4),
            tickets_to_release: LongOpenHashSet::new(),
            main_thread_tasks: VecDeque::new(),
            simulation_distance: 10,
        }
    }
}

impl DistanceManager {
    fn apply_ticket_updates(&mut self, holders: &mut ChunkHolders, updates: Vec<TicketUpdate>) {
        for update in updates {
            if update.simulation {
                let mut levels = SimulationLevels { chunks: &mut self.simulation_chunks, tickets: &self.ticket_storage };
                self.simulation_tracker.update(&mut levels, update.node, update.level, update.only_decreased);
            } else {
                let mut levels = LoadingLevels { holders, tickets: &self.ticket_storage };
                self.loading_tracker.update(&mut levels, update.node, update.level, update.only_decreased);
            }
        }
    }

    pub fn add_ticket(&mut self, holders: &mut ChunkHolders, key: i64, ticket: Ticket) -> bool {
        let mut updates = Vec::new();
        let added = self.ticket_storage.add_ticket(key, ticket, &mut updates);
        self.apply_ticket_updates(holders, updates);
        added
    }

    pub fn remove_ticket(&mut self, holders: &mut ChunkHolders, key: i64, ticket: Ticket) -> bool {
        let mut updates = Vec::new();
        let removed = self.ticket_storage.remove_ticket(key, ticket, &mut updates);
        self.apply_ticket_updates(holders, updates);
        removed
    }

    pub fn add_ticket_with_radius(&mut self, holders: &mut ChunkHolders, ty: TicketType, pos: ChunkPos, radius: i32) {
        let mut updates = Vec::new();
        self.ticket_storage.add_ticket_with_radius(ty, pos.pack(), radius, &mut updates);
        self.apply_ticket_updates(holders, updates);
    }

    pub fn remove_ticket_with_radius(&mut self, holders: &mut ChunkHolders, ty: TicketType, pos: ChunkPos, radius: i32) {
        let mut updates = Vec::new();
        self.ticket_storage.remove_ticket_with_radius(ty, pos.pack(), radius, &mut updates);
        self.apply_ticket_updates(holders, updates);
    }

    pub fn update_chunk_forced(&mut self, holders: &mut ChunkHolders, pos: ChunkPos, forced: bool) -> bool {
        let mut updates = Vec::new();
        let updated = self.ticket_storage.update_chunk_forced(pos, forced, &mut updates);
        self.apply_ticket_updates(holders, updates);
        updated
    }

    pub fn activate_all_deactivated_tickets(&mut self, holders: &mut ChunkHolders) {
        let mut updates = Vec::new();
        self.ticket_storage.activate_all_deactivated_tickets(&mut updates);
        self.apply_ticket_updates(holders, updates);
    }

    pub fn deactivate_tickets_on_closing(&mut self, holders: &mut ChunkHolders) {
        let mut updates = Vec::new();
        self.ticket_storage.deactivate_tickets_on_closing(&mut updates);
        self.apply_ticket_updates(holders, updates);
    }

    pub fn purge_stale_tickets(&mut self, holders: &mut ChunkHolders) {
        let mut updates = Vec::new();
        self.ticket_storage.purge_stale_tickets(
            |pos| holders.map.get(&pos).is_none_or(|h| h.is_ready_for_saving(&holders.futures)),
            &mut updates,
        );
        self.apply_ticket_updates(holders, updates);
    }

    pub fn run_tracker_updates(&mut self, holders: &mut ChunkHolders) -> bool {
        self.run_natural_spawn_updates();
        let mut levels = SimulationLevels { chunks: &mut self.simulation_chunks, tickets: &self.ticket_storage };
        self.simulation_tracker.run_updates(&mut levels, i32::MAX);
        self.run_player_ticket_updates();
        let mut levels = LoadingLevels { holders, tickets: &self.ticket_storage };
        let updates = i32::MAX - self.loading_tracker.run_updates(&mut levels, i32::MAX);
        updates != 0
    }

    fn run_natural_spawn_updates(&mut self) {
        let tracker = &mut self.natural_spawn_chunk_counter;
        let mut levels = FixedLevels { chunks: &mut tracker.chunks, max_distance: tracker.max_distance, players_per_chunk: &self.players_per_chunk, to_update: None };
        tracker.tracker.run_updates(&mut levels, i32::MAX);
    }

    fn run_player_ticket_updates(&mut self) {
        let tracker = &mut self.player_ticket_manager;
        let mut levels =
            FixedLevels { chunks: &mut tracker.chunks, max_distance: tracker.max_distance, players_per_chunk: &self.players_per_chunk, to_update: Some(&mut self.to_update) };
        tracker.tracker.run_updates(&mut levels, i32::MAX);
        if !self.to_update.is_empty() {
            let default_level = self.player_ticket_manager.max_distance + 2;
            let nodes: Vec<i64> = self.to_update.iter().collect();
            for node in nodes {
                let old_level = self.queue_levels.get(&node).copied().unwrap_or(default_level);
                let level = self.player_ticket_manager.chunks.get(node) as i32;
                if old_level != level {
                    self.ticket_dispatcher.on_level_change(node, level);
                    let (saw, sees) = (self.have_ticket_for(old_level), self.have_ticket_for(level));
                    self.on_player_ticket_level_change(node, level, saw, sees);
                }
            }
            self.to_update.clear();
        }
        self.run_dispatcher();
    }

    fn have_ticket_for(&self, level: i32) -> bool {
        level <= self.view_distance
    }

    fn on_player_ticket_level_change(&mut self, key: i64, level: i32, saw: bool, sees: bool) {
        if saw != sees {
            if sees {
                self.ticket_dispatcher.submit(key, level);
            } else {
                self.ticket_dispatcher.release(key, true, true);
            }
        }
    }

    fn run_dispatcher(&mut self) {
        let default_level = self.player_ticket_manager.max_distance + 2;
        self.ticket_dispatcher.run(&mut self.queue_levels, default_level, &mut self.main_thread_tasks);
    }

    pub fn run_main_thread_task(&mut self, holders: &mut ChunkHolders, task: MainThreadTask) {
        let ticket = Ticket::new(TicketType::PlayerLoading, PLAYER_TICKET_LEVEL);
        match task {
            MainThreadTask::Run { pos, last_of_batch } => {
                if self.have_ticket_for(self.player_ticket_manager.chunks.get(pos) as i32) {
                    self.add_ticket(holders, pos, ticket);
                    self.tickets_to_release.add(pos);
                } else {
                    self.ticket_dispatcher.release(pos, false, false);
                }
                if last_of_batch {
                    self.ticket_dispatcher.poll_task();
                }
            }
            MainThreadTask::RemoveTicket(pos) => {
                self.remove_ticket(holders, pos, ticket);
            }
            MainThreadTask::Poll => self.ticket_dispatcher.poll_task(),
            MainThreadTask::ReleasePlayerTicket(pos) => self.ticket_dispatcher.release(pos, false, false),
        }
        self.run_dispatcher();
    }

    pub fn take_tickets_to_release(&mut self) -> Vec<i64> {
        let positions: Vec<i64> = self.tickets_to_release.iter().collect();
        self.tickets_to_release.clear();
        positions
    }

    pub fn has_player_loading_ticket(&self, pos: i64) -> bool {
        self.ticket_storage.tickets(pos).iter().any(|t| t.ty == TicketType::PlayerLoading)
    }

    pub fn add_player(&mut self, holders: &mut ChunkHolders, pos: ChunkPos, player: EntityId) {
        let chunk_pos = pos.pack();
        self.players_per_chunk.entry(chunk_pos).or_default().push(player);
        let tracker = &mut self.natural_spawn_chunk_counter;
        let mut levels = FixedLevels { chunks: &mut tracker.chunks, max_distance: tracker.max_distance, players_per_chunk: &self.players_per_chunk, to_update: None };
        tracker.tracker.update(&mut levels, chunk_pos, 0, true);
        let tracker = &mut self.player_ticket_manager;
        let mut levels =
            FixedLevels { chunks: &mut tracker.chunks, max_distance: tracker.max_distance, players_per_chunk: &self.players_per_chunk, to_update: Some(&mut self.to_update) };
        tracker.tracker.update(&mut levels, chunk_pos, 0, true);
        self.add_ticket(holders, chunk_pos, Ticket::new(TicketType::PlayerSimulation, self.player_ticket_level()));
    }

    pub fn remove_player(&mut self, holders: &mut ChunkHolders, pos: ChunkPos, player: EntityId) {
        let chunk_pos = pos.pack();
        let Some(players) = self.players_per_chunk.get_mut(&chunk_pos) else { return };
        players.retain(|&p| p != player);
        if players.is_empty() {
            self.players_per_chunk.remove(&chunk_pos);
            let tracker = &mut self.natural_spawn_chunk_counter;
            let mut levels = FixedLevels { chunks: &mut tracker.chunks, max_distance: tracker.max_distance, players_per_chunk: &self.players_per_chunk, to_update: None };
            tracker.tracker.update(&mut levels, chunk_pos, i32::MAX, false);
            let tracker = &mut self.player_ticket_manager;
            let mut levels =
                FixedLevels { chunks: &mut tracker.chunks, max_distance: tracker.max_distance, players_per_chunk: &self.players_per_chunk, to_update: Some(&mut self.to_update) };
            tracker.tracker.update(&mut levels, chunk_pos, i32::MAX, false);
            self.remove_ticket(holders, chunk_pos, Ticket::new(TicketType::PlayerSimulation, self.player_ticket_level()));
        }
    }

    fn player_ticket_level(&self) -> i32 {
        0.max(chunk_level::by_full_status(FullChunkStatus::EntityTicking) - self.simulation_distance)
    }

    pub fn in_entity_ticking_range(&self, key: i64) -> bool {
        chunk_level::is_entity_ticking(self.simulation_chunks.get(key) as i32)
    }

    pub fn in_block_ticking_range(&self, key: i64) -> bool {
        chunk_level::is_block_ticking(self.simulation_chunks.get(key) as i32)
    }

    pub fn simulation_level(&self, key: i64) -> i32 {
        self.simulation_chunks.get(key) as i32
    }

    pub fn update_player_tickets(&mut self, view_distance: i32) {
        let entries: Vec<(i64, i8)> = self.player_ticket_manager.chunks.iter().collect();
        for (key, level) in entries {
            let level = level as i32;
            let (saw, sees) = (self.have_ticket_for(level), level <= view_distance);
            self.on_player_ticket_level_change(key, level, saw, sees);
        }
        self.view_distance = view_distance;
        self.run_dispatcher();
    }

    pub fn update_simulation_distance(&mut self, holders: &mut ChunkHolders, new_distance: i32) {
        if new_distance != self.simulation_distance {
            self.simulation_distance = new_distance;
            let mut updates = Vec::new();
            self.ticket_storage.replace_ticket_level_of_type(self.player_ticket_level(), TicketType::PlayerSimulation, &mut updates);
            self.apply_ticket_updates(holders, updates);
        }
    }

    pub fn natural_spawn_chunk_count(&mut self) -> usize {
        self.run_natural_spawn_updates();
        self.natural_spawn_chunk_counter.chunks.len()
    }

    pub fn has_players_nearby(&mut self, pos: i64) -> Option<bool> {
        self.run_natural_spawn_updates();
        let distance = self.natural_spawn_chunk_counter.chunks.get(pos) as i32;
        if distance <= INSCRIBED_SQUARE_SPAWN_DISTANCE_CHUNK {
            Some(true)
        } else if distance > 8 {
            Some(false)
        } else {
            None
        }
    }

    pub fn spawn_candidate_chunks(&mut self) -> Vec<i64> {
        self.run_natural_spawn_updates();
        self.natural_spawn_chunk_counter.chunks.keys().collect()
    }

    pub fn entity_ticking_chunks(&self) -> Vec<i64> {
        self.simulation_chunks.iter().filter(|&(_, level)| chunk_level::is_entity_ticking(level as i32)).map(|(key, _)| key).collect()
    }

    pub fn has_tickets(&self) -> bool {
        self.ticket_storage.has_tickets()
    }

    pub fn ticket_level(&self, pos: ChunkPos, simulation: bool) -> i32 {
        self.ticket_storage.ticket_level_at(pos.pack(), simulation)
    }

    pub fn simulation_levels(&self) -> impl Iterator<Item = (i64, i8)> + '_ {
        self.simulation_chunks.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_all(dm: &mut DistanceManager, holders: &mut ChunkHolders) -> bool {
        let updated = dm.run_tracker_updates(holders);
        if !holders.chunks_to_update_futures.is_empty() {
            holders.chunks_to_update_futures.clear();
            return true;
        }
        for pos in dm.take_tickets_to_release() {
            if dm.has_player_loading_ticket(pos) {
                dm.main_thread_tasks.push_back(MainThreadTask::ReleasePlayerTicket(pos));
            }
        }
        updated
    }

    fn settle(dm: &mut DistanceManager, holders: &mut ChunkHolders) {
        loop {
            if run_all(dm, holders) {
                continue;
            }
            match dm.main_thread_tasks.pop_front() {
                Some(task) => dm.run_main_thread_task(holders, task),
                None => break,
            }
        }
    }

    fn h(h: i64, v: i64) -> i64 {
        h.wrapping_mul(31).wrapping_add(v)
    }

    type State = (&'static str, usize, i64, usize, i64, usize, i64, usize, i64, i64);

    fn state(step: &'static str, dm: &mut DistanceManager, holders: &ChunkHolders) -> State {
        let simulation: Vec<(i64, i8)> = dm.simulation_levels().collect();
        let hs = simulation.iter().fold(0, |acc, &(k, v)| h(h(acc, k), v as i64));
        let ticking = dm.entity_ticking_chunks();
        let candidates = dm.spawn_candidate_chunks();
        let mut keys: Vec<i64> = holders.map.keys().copied().collect();
        keys.sort();
        let hl = keys.iter().fold(0, |acc, k| h(h(acc, *k), holders.map[k].ticket_level as i64));
        let ht = dm.ticket_storage.iter().fold(0, |acc, (k, tickets)| tickets.iter().fold(h(acc, k), |acc, t| h(acc, t.level as i64)));
        (
            step,
            simulation.len(),
            hs,
            ticking.len(),
            ticking.iter().fold(0, |acc, &k| h(acc, k)),
            candidates.len(),
            candidates.iter().fold(0, |acc, &k| h(acc, k)),
            keys.len(),
            hl,
            ht,
        )
    }

    #[test]
    fn levels_and_orders_match_vanilla() {
        let mut dm = DistanceManager::default();
        let mut holders = ChunkHolders::default();
        let (a, b) = (1, 2);
        let mut got = Vec::new();
        dm.update_player_tickets(10);
        settle(&mut dm, &mut holders);
        dm.add_player(&mut holders, ChunkPos::new(0, 0), a);
        settle(&mut dm, &mut holders);
        got.push(state("join", &mut dm, &holders));
        let move_player = |dm: &mut DistanceManager, holders: &mut ChunkHolders, from: (i32, i32), to: (i32, i32)| {
            dm.remove_player(holders, ChunkPos::new(from.0, from.1), a);
            dm.add_player(holders, ChunkPos::new(to.0, to.1), a);
            settle(dm, holders);
        };
        move_player(&mut dm, &mut holders, (0, 0), (1, 0));
        got.push(state("east", &mut dm, &holders));
        move_player(&mut dm, &mut holders, (1, 0), (1, 0));
        got.push(state("up", &mut dm, &holders));
        dm.add_player(&mut holders, ChunkPos::new(-7, 3), b);
        settle(&mut dm, &mut holders);
        got.push(state("second", &mut dm, &holders));
        dm.add_ticket(&mut holders, ChunkPos::new(25, 25).pack(), Ticket::new(TicketType::Unknown, 33));
        settle(&mut dm, &mut holders);
        got.push(state("unknown", &mut dm, &holders));
        for _ in 0..3 {
            dm.purge_stale_tickets(&mut holders);
            settle(&mut dm, &mut holders);
        }
        got.push(state("purged", &mut dm, &holders));
        dm.update_simulation_distance(&mut holders, 6);
        settle(&mut dm, &mut holders);
        got.push(state("simulation 6", &mut dm, &holders));
        dm.update_player_tickets(5);
        settle(&mut dm, &mut holders);
        got.push(state("view 5", &mut dm, &holders));
        move_player(&mut dm, &mut holders, (1, 0), (6, -4));
        got.push(state("far", &mut dm, &holders));
        dm.remove_player(&mut holders, ChunkPos::new(-7, 3), b);
        settle(&mut dm, &mut holders);
        got.push(state("leave", &mut dm, &holders));
        dm.remove_player(&mut holders, ChunkPos::new(6, -4), a);
        settle(&mut dm, &mut holders);
        got.push(state("empty", &mut dm, &holders));
        #[rustfmt::skip]
        let expected: [State; 11] = [
            ("join", 529, -3591164895408119755, 441, 3074046085975810176, 289, 6576588999798052154, 2209, -8142527657234910137, 5660438032601734130),
            ("east", 529, -179868609657020412, 441, -9142791661714528951, 289, -6861249661381301055, 2256, 889713730549198178, 9202527315961757307),
            ("up", 529, -179868609657020412, 441, -9142791661714528951, 289, -6861249661381301055, 2256, 889713730549198178, 9202527315961757307),
            ("second", 758, -3815370934968756088, 648, -7617599991728696148, 452, 5981020677659503864, 2705, 3623371059905935010, 3955822436353183294),
            ("unknown", 758, -3815370934968756088, 648, -7617599991728696148, 452, 5981020677659503864, 3115, 8194094531970279238, -6360219024402236714),
            ("purged", 758, -3815370934968756088, 648, -7617599991728696148, 452, 5981020677659503864, 3115, 6884569906281840828, 3955822436353183294),
            ("simulation 6", 366, 425825703033921228, 288, -3125231188477828600, 452, 5981020677659503864, 3115, 6884569906281840828, -1402064511123807810),
            ("view 5", 366, 425825703033921228, 288, -3125231188477828600, 452, 5981020677659503864, 3115, 5997164607853975608, 7230539959867532414),
            ("far", 434, 4356282154217420907, 338, -7798087043322084351, 538, 2519250551847352911, 3115, 458639482020803418, -5475542664509591687),
            ("leave", 225, 6579986698278569235, 169, -6103955592103055132, 289, -8506398899179241698, 3115, 5759856177251466648, 7714330790551565270),
            ("empty", 0, 0, 0, 0, 0, 0, 3115, -2447286313309858202, 0),
        ];
        for (got, expected) in got.iter().zip(&expected) {
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn player_chunks_tick_by_simulation_distance() {
        let mut dm = DistanceManager::default();
        let mut holders = ChunkHolders::default();
        dm.update_player_tickets(10);
        dm.update_simulation_distance(&mut holders, 4);
        dm.add_player(&mut holders, ChunkPos::new(0, 0), 1);
        settle(&mut dm, &mut holders);
        assert!(dm.in_entity_ticking_range(ChunkPos::new(4, -4).pack()));
        assert!(!dm.in_entity_ticking_range(ChunkPos::new(5, 0).pack()));
        assert!(dm.in_block_ticking_range(ChunkPos::new(5, 0).pack()));
        assert!(!dm.in_block_ticking_range(ChunkPos::new(6, 0).pack()));
        assert_eq!(dm.entity_ticking_chunks().len(), 81);
        assert_eq!(holders.map[&ChunkPos::new(10, 10).pack()].ticket_level, 31);
        assert_eq!(holders.map[&ChunkPos::new(12, 0).pack()].ticket_level, 33);
        assert_eq!(holders.map[&ChunkPos::new(23, 0).pack()].ticket_level, 44);
        assert!(!holders.map.contains_key(&ChunkPos::new(24, 0).pack()));
    }
}
