use crisp_core::{BlockPos, ChunkPos};
use crisp_data::{Block, BlockState, Fluid, FluidState, blocks};

use crate::world::level::biome;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::chunk::{Biome, HeightmapType, LevelChunk};
use crate::world::level::levelgen::AlgorithmRandom;
use crate::world::level::levelgen::material::WorldGenerationContext;
use crate::world::level::levelgen::random_state::RandomState;
use crate::world::level::lighting::has_different_light_properties;
use crate::world::level::update_flags::UPDATE_KNOWN_SHAPE;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader, ScheduledTickAccess};
use crate::world::ticks::TickPriority;

const WORLDGEN_REGION_RANDOM: &str = "minecraft:worldgen_region_random";

pub struct WorldGenRegion<'a> {
    cache: [Option<&'a mut LevelChunk>; 9],
    writable: [bool; 9],
    lit: [bool; 9],
    center: ChunkPos,
    seed: i64,
    biome_zoom_seed: i64,
    sea_level: i32,
    min_y: i32,
    height: i32,
    generation: WorldGenerationContext,
    random: AlgorithmRandom,
    write_radius: i32,
    pub light_tasks: Vec<RegionLightTask>,
}

pub enum RegionLightTask {
    UpdateSectionStatus(BlockPos, bool),
    CheckBlock(BlockPos),
}

impl<'a> WorldGenRegion<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        center: ChunkPos,
        cache: [Option<&'a mut LevelChunk>; 9],
        writable: [bool; 9],
        lit: [bool; 9],
        random_state: &RandomState,
        min_y: i32,
        height: i32,
        generation: WorldGenerationContext,
    ) -> WorldGenRegion<'a> {
        let seed = random_state.seed;
        let random = random_state.random_factory(WORLDGEN_REGION_RANDOM).at(center.min_block_x(), 0, center.min_block_z());
        WorldGenRegion {
            cache,
            writable,
            lit,
            center,
            seed,
            biome_zoom_seed: biome::obfuscate_seed(seed),
            sea_level: generation.sea_level,
            min_y,
            height,
            generation,
            random,
            write_radius: 1,
            light_tasks: Vec::new(),
        }
    }

    fn index(&self, chunk_x: i32, chunk_z: i32) -> usize {
        let (dx, dz) = (chunk_x - self.center.x, chunk_z - self.center.z);
        assert!(dx.abs() <= 1 && dz.abs() <= 1, "Requested chunk unavailable during world generation: {chunk_x}, {chunk_z} around {:?}", self.center);
        ((dz + 1) * 3 + dx + 1) as usize
    }

    pub fn get_center(&self) -> ChunkPos {
        self.center
    }

    pub fn generation(&self) -> WorldGenerationContext {
        self.generation
    }

    pub fn get_seed(&self) -> i64 {
        self.seed
    }

    pub fn get_sea_level(&self) -> i32 {
        self.sea_level
    }

    pub fn get_min_y(&self) -> i32 {
        self.min_y
    }

    pub fn get_height_of_level(&self) -> i32 {
        self.height
    }

    pub fn get_max_y(&self) -> i32 {
        self.min_y + self.height - 1
    }

    pub fn is_outside_build_height(&self, y: i32) -> bool {
        y < self.min_y || y >= self.min_y + self.height
    }

    pub fn get_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> &mut LevelChunk {
        let index = self.index(chunk_x, chunk_z);
        self.cache[index].as_deref_mut().expect("a chunk in the region")
    }

    fn chunk(&self, chunk_x: i32, chunk_z: i32) -> &LevelChunk {
        self.cache[self.index(chunk_x, chunk_z)].as_deref().expect("a chunk in the region")
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        if self.is_outside_build_height(pos.y) {
            return blocks::VOID_AIR.default_state();
        }
        self.chunk(pos.x >> 4, pos.z >> 4).block_state(pos)
    }

    pub fn get_fluid_state(&self, pos: BlockPos) -> FluidState {
        self.get_block_state(pos).fluid_state()
    }

    pub fn get_height(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32 {
        self.get_chunk(x >> 4, z >> 4).get_height(kind, x & 15, z & 15) + 1
    }

    pub fn get_noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        self.chunk(quart_x >> 2, quart_z >> 2).noise_biome(quart_x, quart_y, quart_z)
    }

    pub fn get_biome(&self, pos: BlockPos) -> Biome {
        let (x, y, z) = biome::zoomed_quart(self.biome_zoom_seed, pos.x, pos.y, pos.z);
        self.get_noise_biome(x, y, z)
    }

    fn is_within_write_zone(&self, chunk_x: i32, chunk_z: i32) -> bool {
        (self.center.x - chunk_x).abs() <= self.write_radius && (self.center.z - chunk_z).abs() <= self.write_radius
    }

    pub fn ensure_can_write(&self, pos: BlockPos) -> bool {
        self.is_within_write_zone(pos.x >> 4, pos.z >> 4)
    }

    pub fn set_block(&mut self, pos: BlockPos, state: BlockState, flags: i32) -> bool {
        if !self.ensure_can_write(pos) {
            return false;
        }
        let index = self.index(pos.x >> 4, pos.z >> 4);
        if !self.writable[index] {
            return true;
        }
        let outside = self.is_outside_build_height(pos.y);
        let lit = self.lit[index];
        let chunk = self.cache[index].as_deref_mut().expect("a chunk in the region");
        let old = if outside {
            Some(blocks::VOID_AIR.default_state())
        } else {
            let before = chunk.block_state(pos);
            let section = ((pos.y - chunk.min_y()) >> 4) as usize;
            let was_empty = chunk.sections[section].has_only_air();
            if chunk.set_block_state(pos, state).is_some() && lit {
                let is_empty = chunk.sections[section].has_only_air();
                if is_empty != was_empty {
                    self.light_tasks.push(RegionLightTask::UpdateSectionStatus(pos, is_empty));
                }
                if has_different_light_properties(before, state) {
                    let chunk = self.cache[index].as_deref_mut().expect("a chunk in the region");
                    chunk.update_sky_light_sources(pos);
                    self.light_tasks.push(RegionLightTask::CheckBlock(pos));
                }
            }
            Some(before)
        };
        let chunk = self.cache[index].as_deref_mut().expect("a chunk in the region");
        if state.has_block_entity() {
            chunk.block_entities.pending.insert(pos, dummy_block_entity(pos));
        } else if old.is_some_and(|old| old.has_block_entity()) {
            chunk.block_entities.pending.shift_remove(&pos);
            chunk.block_entities.map.remove(&pos);
        }
        if flags & UPDATE_KNOWN_SHAPE == 0
            && let Some(post_process_pos) = post_process_pos(state, pos)
        {
            self.mark_pos_for_post_processing(post_process_pos);
        }
        true
    }

    pub fn set_section_block_state(&mut self, pos: BlockPos, state: BlockState) {
        let chunk = self.get_chunk(pos.x >> 4, pos.z >> 4);
        let index = ((pos.y - chunk.min_y()) >> 4) as usize;
        chunk.sections[index].set_block_state((pos.x & 15) as usize, (pos.y & 15) as usize, (pos.z & 15) as usize, state);
    }

    pub fn set_block_and_update(&mut self, pos: BlockPos, state: BlockState) -> bool {
        self.set_block(pos, state, crate::world::level::update_flags::UPDATE_ALL)
    }

    pub fn mark_pos_for_post_processing(&mut self, pos: BlockPos) {
        self.get_chunk(pos.x >> 4, pos.z >> 4).mark_pos_for_post_processing(pos);
    }

    pub fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut BlockEntity> {
        let state = self.get_block_state(pos);
        let chunk = self.get_chunk(pos.x >> 4, pos.z >> 4);
        if !chunk.block_entities.map.contains_key(&pos)
            && let Some(tag) = chunk.block_entities.pending.get(&pos)
        {
            let entity = if tag.get("id").and_then(crisp_protocol::nbt::Nbt::as_str) == Some("DUMMY") {
                if state.has_block_entity() { BlockEntity::new_for(pos, state) } else { None }
            } else {
                BlockEntity::load_static(pos, state, tag)
            };
            if let Some(entity) = entity {
                chunk.block_entities.pending.shift_remove(&pos);
                chunk.set_block_entity(entity);
            }
        }
        chunk.block_entities.map.get_mut(&pos)
    }
}

fn dummy_block_entity(pos: BlockPos) -> crisp_protocol::nbt::Nbt {
    use crisp_protocol::nbt::Nbt;
    Nbt::Compound(vec![("x".into(), Nbt::Int(pos.x)), ("y".into(), Nbt::Int(pos.y)), ("z".into(), Nbt::Int(pos.z)), ("id".into(), Nbt::String("DUMMY".into()))])
}

fn post_process_pos(state: BlockState, pos: BlockPos) -> Option<BlockPos> {
    let block = state.block();
    if block == blocks::BROWN_MUSHROOM || block == blocks::RED_MUSHROOM {
        Some(pos)
    } else if block == blocks::SOUL_SAND || block == blocks::MAGMA_BLOCK {
        Some(pos.above())
    } else {
        None
    }
}

impl BlockGetter for WorldGenRegion<'_> {
    fn block_state(&mut self, pos: BlockPos) -> BlockState {
        self.get_block_state(pos)
    }
}

impl LevelReader for WorldGenRegion<'_> {
    fn raw_brightness(&mut self, _pos: BlockPos, _darkening: i32) -> i32 {
        0
    }

    fn is_fast_lava(&self) -> bool {
        false
    }

    fn get_control_input_signal(&mut self, pos: BlockPos, direction: crisp_core::Direction, only_diodes: bool) -> i32 {
        crate::world::level::signal::get_control_input_signal(self, pos, direction, only_diodes)
    }

    fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut BlockEntity> {
        WorldGenRegion::get_block_entity(self, pos)
    }
}

impl ScheduledTickAccess for WorldGenRegion<'_> {
    fn schedule_tick_with_priority(&mut self, pos: BlockPos, block: Block, _delay: i32, priority: TickPriority) {
        if let Some(proto) = self.get_chunk(pos.x >> 4, pos.z >> 4).proto.as_deref_mut() {
            proto.block_ticks.schedule(block, pos, priority);
        }
    }

    fn schedule_fluid_tick(&mut self, pos: BlockPos, fluid: Fluid, _delay: i32) {
        if let Some(proto) = self.get_chunk(pos.x >> 4, pos.z >> 4).proto.as_deref_mut() {
            proto.fluid_ticks.schedule(fluid, pos, TickPriority::NORMAL);
        }
    }

    fn has_scheduled_tick(&self, pos: BlockPos, block: Block) -> bool {
        self.chunk(pos.x >> 4, pos.z >> 4).proto.as_deref().is_some_and(|proto| proto.block_ticks.has_scheduled_tick(pos, block))
    }
}

impl LevelAccessor for WorldGenRegion<'_> {
    fn get_random(&mut self) -> &mut dyn crisp_core::random::RandomSource {
        &mut self.random
    }
}
