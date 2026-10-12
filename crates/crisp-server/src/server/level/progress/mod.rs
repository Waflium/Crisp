use std::time::Instant;

use crate::network::chat::{Arg, Component};

const PREPARE_SERVER_WEIGHT: i32 = 10;
const EXPECTED_PLAYER_CHUNKS: i32 = 7 * 7;
const LOG_INTERVAL_MILLIS: u128 = 500;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    StartServer,
    PrepareGlobalSpawn,
    LoadInitialChunks,
    LoadPlayerChunks,
}

#[derive(Default)]
struct LevelLoadProgressTracker {
    include_player_chunks: bool,
    total_weight: i32,
    finalized_weight: i32,
    segment_weight: i32,
    segment_fraction: f32,
    progress: f32,
}

impl LevelLoadProgressTracker {
    fn start(&mut self, stage: Stage, total_chunks: i32) {
        if !self.tracks_stage(stage) {
            return;
        }
        match stage {
            Stage::LoadInitialChunks => {
                let player_chunks_weight = if self.include_player_chunks { EXPECTED_PLAYER_CHUNKS } else { 0 };
                self.total_weight = PREPARE_SERVER_WEIGHT + total_chunks + player_chunks_weight;
                self.begin_segment(PREPARE_SERVER_WEIGHT);
                self.finish_segment();
                self.begin_segment(total_chunks);
            }
            Stage::LoadPlayerChunks => self.begin_segment(EXPECTED_PLAYER_CHUNKS),
            _ => {}
        }
    }

    fn begin_segment(&mut self, weight: i32) {
        self.segment_weight = weight;
        self.segment_fraction = 0.0;
        self.update_progress();
    }

    fn update(&mut self, stage: Stage, current_chunks: i32, total_chunks: i32) {
        if self.tracks_stage(stage) {
            self.segment_fraction = if total_chunks == 0 { 0.0 } else { current_chunks as f32 / total_chunks as f32 };
            self.update_progress();
        }
    }

    fn finish(&mut self, stage: Stage) {
        if self.tracks_stage(stage) {
            self.finish_segment();
        }
    }

    fn finish_segment(&mut self) {
        self.finalized_weight += self.segment_weight;
        self.segment_weight = 0;
        self.update_progress();
    }

    fn tracks_stage(&self, stage: Stage) -> bool {
        match stage {
            Stage::LoadInitialChunks => true,
            Stage::LoadPlayerChunks => self.include_player_chunks,
            _ => false,
        }
    }

    fn update_progress(&mut self) {
        self.progress = if self.total_weight == 0 {
            0.0
        } else {
            (self.finalized_weight as f32 + self.segment_fraction * self.segment_weight as f32) / self.total_weight as f32
        };
    }
}

pub struct LoggingLevelLoadListener {
    include_player_chunks: bool,
    tracker: LevelLoadProgressTracker,
    closed: bool,
    epoch: Instant,
    start_time: Option<u128>,
    next_log_time: Option<u128>,
}

impl LoggingLevelLoadListener {
    pub fn for_dedicated_server() -> LoggingLevelLoadListener {
        LoggingLevelLoadListener {
            include_player_chunks: false,
            tracker: LevelLoadProgressTracker::default(),
            closed: false,
            epoch: Instant::now(),
            start_time: None,
            next_log_time: None,
        }
    }

    fn millis(&self) -> u128 {
        self.epoch.elapsed().as_millis()
    }

    pub fn start(&mut self, stage: Stage, total_chunks: i32) {
        if self.closed {
            return;
        }
        if self.start_time.is_none() {
            let now = self.millis();
            self.start_time = Some(now);
            self.next_log_time = Some(now);
        }
        self.tracker.start(stage, total_chunks);
        match stage {
            Stage::PrepareGlobalSpawn => info!("Selecting global world spawn..."),
            Stage::LoadInitialChunks => info!("Loading {total_chunks} persistent chunks..."),
            Stage::LoadPlayerChunks => info!("Loading {total_chunks} chunks for player spawn..."),
            Stage::StartServer => {}
        }
    }

    pub fn update(&mut self, stage: Stage, current_chunks: i32, total_chunks: i32) {
        if self.closed {
            return;
        }
        self.tracker.update(stage, current_chunks, total_chunks);
        if let Some(next) = self.next_log_time
            && self.millis() > next
        {
            self.next_log_time = Some(next + LOG_INTERVAL_MILLIS);
            let percent = (self.tracker.progress * 100.0).floor() as i32;
            info!("{}", Component::translatable_with("menu.preparingSpawn", vec![Arg::Int(percent)]).get_string());
        }
    }

    pub fn finish(&mut self, stage: Stage) {
        if self.closed {
            return;
        }
        self.tracker.finish(stage);
        let final_stage = if self.include_player_chunks { Stage::LoadPlayerChunks } else { Stage::LoadInitialChunks };
        if stage == final_stage {
            info!("Time elapsed: {} ms", self.millis() - self.start_time.unwrap_or(0));
            self.next_log_time = None;
            self.closed = true;
        }
    }
}

#[derive(Default)]
pub struct ChunkLoadCounter {
    pending_chunks: Vec<(usize, i64)>,
    total_chunks: i32,
}

impl ChunkLoadCounter {
    pub fn track(&mut self, level: usize, chunks: &mut super::chunk_cache::ChunkCache, scheduler: impl FnOnce(&mut super::chunk_cache::ChunkCache)) {
        chunks.run_distance_manager_updates();
        let already_loaded: std::collections::HashSet<i64> = chunks.chunks_with_at_least_full_status().into_iter().collect();
        scheduler(chunks);
        chunks.run_distance_manager_updates();
        for key in chunks.chunks_with_at_least_full_status() {
            if !already_loaded.contains(&key) {
                self.pending_chunks.push((level, key));
                self.total_chunks += 1;
            }
        }
    }

    pub fn ready_chunks(&mut self, levels: &[super::ServerLevel]) -> i32 {
        self.total_chunks - self.pending_chunks(levels)
    }

    pub fn pending_chunks(&mut self, levels: &[super::ServerLevel]) -> i32 {
        self.pending_chunks.retain(|&(level, key)| !levels[level].chunks.latest_status_is_full(key));
        self.pending_chunks.len() as i32
    }

    pub fn total_chunks(&self) -> i32 {
        self.total_chunks
    }
}
