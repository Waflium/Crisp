use std::cell::Cell;
use std::collections::VecDeque;

use crisp_core::{BlockPos, ChunkPos};
use crisp_data::{BlockState, blocks};

use super::section_storage::LayerLightSectionStorage;
use super::{LightChunkGetter, NodeSet, section};
use crate::world::level::chunk::LevelChunk;
use crate::world::level::chunk::data_layer::DataLayer;
use crate::world::level::light_layer::LightLayer;

pub mod queue_entry {
    use crisp_core::Direction;

    const LEVEL_MASK: i64 = 15;
    const DIRECTIONS_MASK: i64 = 1008;
    const FLAG_FROM_EMPTY_SHAPE: i64 = 1024;
    const FLAG_INCREASE_FROM_EMISSION: i64 = 2048;

    pub const fn decrease_skip_one_direction(old_from_level: i32, skip: Direction) -> i64 {
        with_level(without_direction(DIRECTIONS_MASK, skip), old_from_level)
    }

    pub const fn decrease_all_directions(old_from_level: i32) -> i64 {
        with_level(DIRECTIONS_MASK, old_from_level)
    }

    pub const fn increase_light_from_emission(new_from_level: i32, from_empty_shape: bool) -> i64 {
        let mut data = DIRECTIONS_MASK | FLAG_INCREASE_FROM_EMISSION;
        if from_empty_shape {
            data |= FLAG_FROM_EMPTY_SHAPE;
        }
        with_level(data, new_from_level)
    }

    pub const fn increase_skip_one_direction(new_from_level: i32, from_empty_shape: bool, skip: Direction) -> i64 {
        let mut data = without_direction(DIRECTIONS_MASK, skip);
        if from_empty_shape {
            data |= FLAG_FROM_EMPTY_SHAPE;
        }
        with_level(data, new_from_level)
    }

    pub const fn increase_only_one_direction(new_from_level: i32, from_empty_shape: bool, direction: Direction) -> i64 {
        let mut data = 0;
        if from_empty_shape {
            data |= FLAG_FROM_EMPTY_SHAPE;
        }
        with_level(with_direction(data, direction), new_from_level)
    }

    pub const fn increase_sky_source_in_directions(down: bool, north: bool, south: bool, west: bool, east: bool) -> i64 {
        let mut data = with_level(0, 15);
        if down {
            data = with_direction(data, Direction::Down);
        }
        if north {
            data = with_direction(data, Direction::North);
        }
        if south {
            data = with_direction(data, Direction::South);
        }
        if west {
            data = with_direction(data, Direction::West);
        }
        if east {
            data = with_direction(data, Direction::East);
        }
        data
    }

    pub const fn from_level(entry: i64) -> i32 {
        (entry & LEVEL_MASK) as i32
    }

    pub const fn is_from_empty_shape(entry: i64) -> bool {
        entry & FLAG_FROM_EMPTY_SHAPE != 0
    }

    pub const fn is_increase_from_emission(entry: i64) -> bool {
        entry & FLAG_INCREASE_FROM_EMISSION != 0
    }

    pub const fn should_propagate_in_direction(entry: i64, direction: Direction) -> bool {
        entry & 1 << (direction.ordinal() + 4) != 0
    }

    const fn with_level(entry: i64, level: i32) -> i64 {
        entry & -16 | level as i64 & LEVEL_MASK
    }

    const fn with_direction(entry: i64, direction: Direction) -> i64 {
        entry | 1 << (direction.ordinal() + 4)
    }

    const fn without_direction(entry: i64, direction: Direction) -> i64 {
        entry & !(1 << (direction.ordinal() + 4))
    }
}

pub const PULL_LIGHT_IN_ENTRY: i64 = queue_entry::decrease_all_directions(1);

pub struct LightEngine {
    pub storage: LayerLightSectionStorage,
    block_nodes_to_check: NodeSet,
    decrease_queue: VecDeque<(i64, i64)>,
    increase_queue: VecDeque<(i64, i64)>,
}

impl LightEngine {
    pub fn new(layer: LightLayer) -> LightEngine {
        LightEngine {
            storage: LayerLightSectionStorage::new(layer),
            block_nodes_to_check: NodeSet::default(),
            decrease_queue: VecDeque::with_capacity(512),
            increase_queue: VecDeque::with_capacity(512),
        }
    }

    pub fn check_block(&mut self, pos: BlockPos) {
        self.block_nodes_to_check.insert(pos.as_long());
    }

    pub fn queue_section_data(&mut self, section_node: i64, data: Option<DataLayer>) {
        self.storage.queue_section_data(section_node, data);
    }

    pub fn retain_data(&mut self, pos: ChunkPos, retain: bool) {
        self.storage.retain_data(section::zero_node(pos.x, pos.z), retain);
    }

    pub fn update_section_status(&mut self, section_node: i64, section_empty: bool) {
        self.storage.update_section_status(section_node, section_empty);
    }

    pub fn set_light_enabled(&mut self, pos: ChunkPos, enable: bool) {
        self.storage.set_light_enabled(section::zero_node(pos.x, pos.z), enable);
    }

    pub fn enqueue_decrease(&mut self, from_node: i64, decrease_data: i64) {
        self.decrease_queue.push_back((from_node, decrease_data));
    }

    pub fn enqueue_increase(&mut self, from_node: i64, increase_data: i64) {
        self.increase_queue.push_back((from_node, increase_data));
    }

    pub fn has_light_work(&self) -> bool {
        self.storage.has_inconsistencies() || !self.block_nodes_to_check.is_empty() || !self.decrease_queue.is_empty() || !self.increase_queue.is_empty()
    }
}

pub trait Propagator {
    fn base(&mut self) -> &mut LightEngine;

    fn check_node<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, block_node: i64);

    fn propagate_increase<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, from_node: i64, increase_data: i64, from_level: i32);

    fn propagate_decrease<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, from_node: i64, decrease_data: i64);
}

pub fn run_light_updates<E: Propagator, C: LightChunkGetter + ?Sized>(engine: &mut E, chunks: &C, changes: &mut Vec<(LightLayer, i64)>) -> i32 {
    let mut to_check = std::mem::take(&mut engine.base().block_nodes_to_check);
    for &node in &to_check {
        engine.check_node(chunks, node);
    }
    to_check.clear();
    engine.base().block_nodes_to_check = to_check;
    let mut count = 0;
    while let Some((from_node, decrease_data)) = engine.base().decrease_queue.pop_front() {
        engine.propagate_decrease(chunks, from_node, decrease_data);
        count += 1;
    }
    while let Some((from_node, increase_data)) = engine.base().increase_queue.pop_front() {
        let base = engine.base();
        let mut from_level = base.storage.stored_level(from_node) as i32;
        let from_target_level = queue_entry::from_level(increase_data);
        if queue_entry::is_increase_from_emission(increase_data) && from_level < from_target_level {
            base.storage.set_stored_level(from_node, from_target_level as u8);
            from_level = from_target_level;
        }
        if from_level == from_target_level {
            engine.propagate_increase(chunks, from_node, increase_data, from_level);
        }
        count += 1;
    }
    let storage = &mut engine.base().storage;
    storage.mark_new_inconsistencies();
    storage.swap_section_map(changes);
    count
}

pub fn state_at<C: LightChunkGetter + ?Sized>(chunks: &C, block_node: i64) -> BlockState {
    let pos = BlockPos::of_long(block_node);
    match chunks.chunk_for_lighting(pos.x >> 4, pos.z >> 4) {
        Some(chunk) => chunk.block_state(pos),
        None => blocks::BEDROCK.default_state(),
    }
}

pub fn opacity(state: BlockState) -> i32 {
    (state.light_dampening() as i32).max(1)
}

pub struct CachedChunks<'a, C: LightChunkGetter + ?Sized> {
    chunks: &'a C,
    last: Cell<[(i64, Option<&'a LevelChunk>); 2]>,
}

impl<'a, C: LightChunkGetter + ?Sized> CachedChunks<'a, C> {
    pub fn new(chunks: &'a C) -> CachedChunks<'a, C> {
        CachedChunks { chunks, last: Cell::new([(i64::MIN, None); 2]) }
    }
}

impl<C: LightChunkGetter + ?Sized> LightChunkGetter for CachedChunks<'_, C> {
    fn chunk_for_lighting(&self, x: i32, z: i32) -> Option<&LevelChunk> {
        let key = ChunkPos::new(x, z).pack();
        let last = self.last.get();
        for (pos, chunk) in last {
            if pos == key {
                return chunk;
            }
        }
        let chunk = self.chunks.chunk_for_lighting(x, z);
        self.last.set([(key, chunk), last[0]]);
        chunk
    }
}
