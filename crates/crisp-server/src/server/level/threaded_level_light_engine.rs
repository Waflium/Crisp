use crisp_core::hash_map::FastMap;
use std::collections::VecDeque;

use crisp_core::{BlockPos, ChunkPos};

use crate::world::level::chunk::LevelChunk;
use crate::world::level::chunk::data_layer::DataLayer;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::lighting::{LevelLightEngine, section};

const TASKS_PER_BATCH: usize = 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskType {
    PreUpdate,
    PostUpdate,
}

pub enum LightTask {
    CheckBlock(BlockPos),
    UpdateSectionStatus(i64, bool),
    QueueSectionData(LightLayer, i64, Option<DataLayer>),
    RetainData(ChunkPos, bool),
    InitializeLight(ChunkPos),
    InitializeLightDone { pos: ChunkPos, lighted: bool, done: u64 },
    LightChunk { pos: ChunkPos, lighted: bool },
    LightChunkDone { pos: ChunkPos, done: u64 },
    UpdateChunkStatus(ChunkPos),
}

pub struct ThreadedLevelLightEngine {
    pub engine: LevelLightEngine,
    light_tasks: VecDeque<(TaskType, LightTask)>,
    min_section: i32,
    max_section: i32,
}

impl ThreadedLevelLightEngine {
    pub fn new(min_y: i32, height: i32, has_sky_light: bool) -> ThreadedLevelLightEngine {
        ThreadedLevelLightEngine {
            engine: LevelLightEngine::new(min_y, height, true, has_sky_light),
            light_tasks: VecDeque::new(),
            min_section: min_y >> 4,
            max_section: ((min_y + height) >> 4) - 1,
        }
    }

    pub fn add_task(&mut self, chunks: &mut FastMap<ChunkPos, LevelChunk>, ty: TaskType, task: LightTask, changes: &mut Vec<(LightLayer, i64)>, done: &mut Vec<u64>) {
        self.light_tasks.push_back((ty, task));
        if self.light_tasks.len() >= TASKS_PER_BATCH {
            self.run_update(chunks, changes, done);
        }
    }

    pub fn has_work(&self) -> bool {
        !self.light_tasks.is_empty() || self.engine.has_light_work()
    }

    pub fn run_update(&mut self, chunks: &mut FastMap<ChunkPos, LevelChunk>, changes: &mut Vec<(LightLayer, i64)>, done: &mut Vec<u64>) {
        let total = self.light_tasks.len().min(TASKS_PER_BATCH);
        let mut batch: Vec<(TaskType, LightTask)> = self.light_tasks.drain(..total).collect();
        let mut correct = Vec::new();
        {
            let chunks: &FastMap<ChunkPos, LevelChunk> = chunks;
            for (ty, task) in &mut batch {
                if *ty == TaskType::PreUpdate {
                    self.run_task(chunks, task, &mut correct, done);
                }
            }
            self.engine.run_light_updates(chunks, changes);
            for (ty, task) in &mut batch {
                if *ty == TaskType::PostUpdate {
                    self.run_task(chunks, task, &mut correct, done);
                }
            }
        }
        for pos in correct {
            if let Some(chunk) = chunks.get_mut(&pos) {
                chunk.light_correct = true;
            }
        }
    }

    fn run_task(&mut self, chunks: &FastMap<ChunkPos, LevelChunk>, task: &mut LightTask, correct: &mut Vec<ChunkPos>, done: &mut Vec<u64>) {
        let engine = &mut self.engine;
        if let LightTask::QueueSectionData(layer, section_node, data) = task {
            engine.queue_section_data(*layer, *section_node, data.take());
            return;
        }
        match *task {
            LightTask::CheckBlock(pos) => engine.check_block(pos),
            LightTask::UpdateSectionStatus(section_node, empty) => engine.update_section_status(section_node, empty),
            LightTask::QueueSectionData(..) => {}
            LightTask::RetainData(pos, retain) => engine.retain_data(pos, retain),
            LightTask::InitializeLight(pos) => {
                let Some(chunk) = chunks.get(&pos) else { return };
                let min_section_y = chunk.min_y() >> 4;
                for (index, s) in chunk.sections.iter().enumerate() {
                    if !s.has_only_air() {
                        engine.update_section_status(section::of(pos.x, min_section_y + index as i32, pos.z), false);
                    }
                }
            }
            LightTask::InitializeLightDone { pos, lighted, done: future } => {
                engine.set_light_enabled(chunks, pos, lighted);
                engine.retain_data(pos, false);
                done.push(future);
            }
            LightTask::LightChunk { pos, lighted } => {
                if !lighted {
                    engine.propagate_light_sources(chunks, pos);
                }
            }
            LightTask::LightChunkDone { pos, done: future } => {
                correct.push(pos);
                done.push(future);
            }
            LightTask::UpdateChunkStatus(pos) => {
                engine.retain_data(pos, false);
                engine.set_light_enabled(chunks, pos, false);
                for section_y in engine.min_light_section()..engine.max_light_section() {
                    engine.queue_section_data(LightLayer::Block, section::of(pos.x, section_y, pos.z), None);
                    engine.queue_section_data(LightLayer::Sky, section::of(pos.x, section_y, pos.z), None);
                }
                for section_y in self.min_section..=self.max_section {
                    engine.update_section_status(section::of(pos.x, section_y, pos.z), true);
                }
            }
        }
    }
}
