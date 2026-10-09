use crisp_core::{BlockPos, ChunkPos};

use super::block_light_engine::BlockLightEngine;
use super::light_engine::{CachedChunks, run_light_updates};
use super::sky_light_engine::SkyLightEngine;
use super::LightChunkGetter;
use crate::world::level::chunk::data_layer::DataLayer;
use crate::world::level::light_layer::LightLayer;

pub struct LevelLightEngine {
    min_section_y: i32,
    section_count: i32,
    block_engine: Option<BlockLightEngine>,
    sky_engine: Option<SkyLightEngine>,
}

impl LevelLightEngine {
    pub fn new(min_y: i32, height: i32, has_block_light: bool, has_sky_light: bool) -> LevelLightEngine {
        LevelLightEngine {
            min_section_y: min_y >> 4,
            section_count: height >> 4,
            block_engine: has_block_light.then(BlockLightEngine::new),
            sky_engine: has_sky_light.then(|| SkyLightEngine::new(min_y)),
        }
    }

    pub fn check_block(&mut self, pos: BlockPos) {
        if let Some(engine) = &mut self.block_engine {
            engine.base.check_block(pos);
        }
        if let Some(engine) = &mut self.sky_engine {
            engine.base.check_block(pos);
        }
    }

    pub fn has_light_work(&self) -> bool {
        self.sky_engine.as_ref().is_some_and(|e| e.base.has_light_work()) || self.block_engine.as_ref().is_some_and(|e| e.base.has_light_work())
    }

    pub fn run_light_updates<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, changes: &mut Vec<(LightLayer, i64)>) -> i32 {
        let mut count = 0;
        if let Some(engine) = &mut self.block_engine {
            count += run_light_updates(engine, &CachedChunks::new(chunks), changes);
        }
        if let Some(engine) = &mut self.sky_engine {
            count += run_light_updates(engine, &CachedChunks::new(chunks), changes);
        }
        count
    }

    pub fn update_section_status(&mut self, section_node: i64, section_empty: bool) {
        if let Some(engine) = &mut self.block_engine {
            engine.base.update_section_status(section_node, section_empty);
        }
        if let Some(engine) = &mut self.sky_engine {
            engine.base.update_section_status(section_node, section_empty);
        }
    }

    pub fn set_light_enabled<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, pos: ChunkPos, enable: bool) {
        if let Some(engine) = &mut self.block_engine {
            engine.base.set_light_enabled(pos, enable);
        }
        if let Some(engine) = &mut self.sky_engine {
            engine.set_light_enabled(chunks, pos, enable);
        }
    }

    pub fn propagate_light_sources<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, pos: ChunkPos) {
        if let Some(engine) = &mut self.block_engine {
            engine.propagate_light_sources(chunks, pos);
        }
        if let Some(engine) = &mut self.sky_engine {
            engine.propagate_light_sources(chunks, pos);
        }
    }

    pub fn queue_section_data(&mut self, layer: LightLayer, section_node: i64, data: Option<DataLayer>) {
        let engine = match layer {
            LightLayer::Block => self.block_engine.as_mut().map(|e| &mut e.base),
            LightLayer::Sky => self.sky_engine.as_mut().map(|e| &mut e.base),
        };
        if let Some(engine) = engine {
            engine.queue_section_data(section_node, data);
        }
    }

    pub fn retain_data(&mut self, pos: ChunkPos, retain: bool) {
        if let Some(engine) = &mut self.block_engine {
            engine.base.retain_data(pos, retain);
        }
        if let Some(engine) = &mut self.sky_engine {
            engine.base.retain_data(pos, retain);
        }
    }

    pub fn data_layer_data(&self, layer: LightLayer, section_node: i64) -> Option<&DataLayer> {
        match layer {
            LightLayer::Block => self.block_engine.as_ref()?.base.storage.data_layer_data(section_node),
            LightLayer::Sky => self.sky_engine.as_ref()?.base.storage.data_layer_data(section_node),
        }
    }

    pub fn light_value(&self, layer: LightLayer, pos: BlockPos) -> i32 {
        match layer {
            LightLayer::Block => self.block_engine.as_ref().map_or(0, |e| e.base.storage.block_light_value(pos.as_long()) as i32),
            LightLayer::Sky => self.sky_engine.as_ref().map_or(0, |e| e.base.storage.sky_light_value(pos.as_long()) as i32),
        }
    }

    pub fn raw_brightness(&self, pos: BlockPos, sky_dampen: i32) -> i32 {
        let sky = if self.sky_engine.is_some() { self.light_value(LightLayer::Sky, pos) - sky_dampen } else { 0 };
        let block = self.light_value(LightLayer::Block, pos);
        block.max(sky)
    }

    pub fn light_section_count(&self) -> i32 {
        self.section_count + 2
    }

    pub fn min_light_section(&self) -> i32 {
        self.min_section_y - 1
    }

    pub fn max_light_section(&self) -> i32 {
        self.min_light_section() + self.light_section_count()
    }
}
