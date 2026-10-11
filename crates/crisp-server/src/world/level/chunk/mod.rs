pub mod block_entities;
pub mod carving_mask;
pub mod chunk_generator_structure_state;
pub mod data_layer;
mod heightmap;
pub mod palette;
pub mod proto_chunk;
pub mod serialization;
pub mod storage;

use crisp_core::{BlockPos, ChunkPos};
use crisp_data::{BlockState, DimensionType, SyncedRegistry, blocks};
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

pub use heightmap::{Heightmap, HeightmapType};
use palette::{PaletteValue, PalettedContainer, Strategy};

use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_start::StructureStart;
use crate::world::level::lighting::sky_light_sources::{ChunkBlocks, ChunkSkyLightSources};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Biome(pub u16);

impl Biome {
    pub fn by_name(name: &str) -> Option<Biome> {
        SyncedRegistry::get("minecraft:worldgen/biome").id_of(name).map(|id| Biome(id as u16))
    }

    pub fn name(self) -> &'static str {
        SyncedRegistry::get("minecraft:worldgen/biome").entries[self.0 as usize]
    }

    fn count() -> usize {
        SyncedRegistry::get("minecraft:worldgen/biome").entries.len()
    }
}

impl PaletteValue for BlockState {
    fn global_id(self) -> u32 {
        self.id()
    }
    fn from_global_id(id: u32) -> Option<BlockState> {
        BlockState::by_id(id)
    }
}

impl PaletteValue for Biome {
    fn global_id(self) -> u32 {
        self.0 as u32
    }
    fn from_global_id(id: u32) -> Option<Biome> {
        ((id as usize) < Biome::count()).then_some(Biome(id as u16))
    }
}

pub fn block_strategy() -> Strategy {
    Strategy::block_states(BlockState::count())
}

pub fn biome_strategy() -> Strategy {
    Strategy::biomes(Biome::count())
}

#[derive(Clone)]
pub struct LevelChunkSection {
    non_empty_block_count: i16,
    fluid_count: i16,
    ticking_block_count: i16,
    ticking_fluid_count: i16,
    pub states: PalettedContainer<BlockState>,
    pub biomes: PalettedContainer<Biome>,
}

impl LevelChunkSection {
    pub fn new(biome: Biome) -> LevelChunkSection {
        LevelChunkSection {
            non_empty_block_count: 0,
            fluid_count: 0,
            ticking_block_count: 0,
            ticking_fluid_count: 0,
            states: PalettedContainer::new(blocks::AIR.default_state(), block_strategy()),
            biomes: PalettedContainer::new(biome, biome_strategy()),
        }
    }

    pub fn from_containers(states: PalettedContainer<BlockState>, biomes: PalettedContainer<Biome>) -> LevelChunkSection {
        let mut section =
            LevelChunkSection { non_empty_block_count: 0, fluid_count: 0, ticking_block_count: 0, ticking_fluid_count: 0, states, biomes };
        section.recalc_block_counts();
        section
    }

    pub fn block_state(&self, x: usize, y: usize, z: usize) -> BlockState {
        self.states.get(x, y, z)
    }

    pub fn set_block_state(&mut self, x: usize, y: usize, z: usize, state: BlockState) -> BlockState {
        let previous = self.states.get_and_set(x, y, z, state);
        if !previous.is_air() {
            self.non_empty_block_count -= 1;
            if previous.is_randomly_ticking() {
                self.ticking_block_count -= 1;
            }
            let fluid = previous.fluid_state();
            if !fluid.is_empty() {
                self.fluid_count -= 1;
                if fluid.is_randomly_ticking() {
                    self.ticking_fluid_count -= 1;
                }
            }
        }
        if !state.is_air() {
            self.non_empty_block_count += 1;
            if state.is_randomly_ticking() {
                self.ticking_block_count += 1;
            }
            let fluid = state.fluid_state();
            if !fluid.is_empty() {
                self.fluid_count += 1;
                if fluid.is_randomly_ticking() {
                    self.ticking_fluid_count += 1;
                }
            }
        }
        previous
    }

    pub fn has_only_air(&self) -> bool {
        self.non_empty_block_count == 0
    }

    pub fn has_fluid(&self) -> bool {
        self.fluid_count > 0
    }

    pub fn is_randomly_ticking(&self) -> bool {
        self.ticking_block_count > 0 || self.ticking_fluid_count > 0
    }

    pub fn recalc_block_counts(&mut self) {
        let (mut non_empty, mut fluids, mut ticking, mut ticking_fluids) = (0, 0, 0, 0);
        for (state, count) in self.states.count() {
            if state.is_air() {
                continue;
            }
            non_empty += count;
            if state.is_randomly_ticking() {
                ticking += count;
            }
            let fluid = state.fluid_state();
            if !fluid.is_empty() {
                fluids += count;
                if fluid.is_randomly_ticking() {
                    ticking_fluids += count;
                }
            }
        }
        self.non_empty_block_count = non_empty as i16;
        self.fluid_count = fluids as i16;
        self.ticking_block_count = ticking as i16;
        self.ticking_fluid_count = ticking_fluids as i16;
    }

    pub fn write(&self, w: &mut Writer) {
        w.i16(self.non_empty_block_count);
        w.i16(self.fluid_count);
        self.states.write(w);
        self.biomes.write(w);
    }
}

#[derive(Default)]
pub struct ChunkStructures {
    pub starts: std::collections::BTreeMap<StructureHolder, std::sync::Arc<StructureStart>>,
    pub references: std::collections::BTreeMap<StructureHolder, crisp_core::fastutil::LongOpenHashSet>,
}

impl ChunkStructures {
    pub fn get_start_for_structure(&self, structure: StructureHolder) -> Option<&std::sync::Arc<StructureStart>> {
        self.starts.get(&structure)
    }

    pub fn set_start_for_structure(&mut self, structure: StructureHolder, start: std::sync::Arc<StructureStart>) {
        self.starts.insert(structure, start);
    }

    pub fn get_references_for_structure(&self, structure: StructureHolder) -> Option<&crisp_core::fastutil::LongOpenHashSet> {
        self.references.get(&structure)
    }

    pub fn add_reference_for_structure(&mut self, structure: StructureHolder, reference: i64) {
        self.references.entry(structure).or_insert_with(crisp_core::fastutil::LongOpenHashSet::new).add(reference);
    }
}

pub struct LevelChunk {
    pub pos: ChunkPos,
    min_y: i32,
    pub sections: Vec<LevelChunkSection>,
    pub heightmaps: [Heightmap; 4],
    pub unsaved: bool,
    pub inhabited_time: i64,
    pub sky_light_sources: ChunkSkyLightSources,
    pub light_correct: bool,
    pub block_entities: block_entities::ChunkBlockEntities,
    pub loaded: bool,
    pub proto: Option<Box<proto_chunk::ProtoChunkData>>,
    pub structures: ChunkStructures,
    pub post_processing: Vec<Vec<u16>>,
}

impl LevelChunk {
    pub fn empty(pos: ChunkPos, dimension: &DimensionType, biome: Biome) -> LevelChunk {
        let sections = (0..dimension.height / 16).map(|_| LevelChunkSection::new(biome)).collect();
        LevelChunk::with_sections(pos, dimension, sections)
    }

    pub fn with_sections(pos: ChunkPos, dimension: &DimensionType, sections: Vec<LevelChunkSection>) -> LevelChunk {
        let heightmaps = HeightmapType::LIVE.map(|t| Heightmap::new(t, dimension.min_y, dimension.height as i32));
        LevelChunk {
            pos,
            min_y: dimension.min_y,
            post_processing: vec![Vec::new(); sections.len()],
            sections,
            heightmaps,
            unsaved: false,
            inhabited_time: 0,
            sky_light_sources: ChunkSkyLightSources::new(dimension.min_y),
            light_correct: false,
            block_entities: Default::default(),
            loaded: false,
            proto: None,
            structures: ChunkStructures::default(),
        }
    }

    pub fn get_height(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32 {
        let top = self.highest_section_position() + 16;
        if let Some(proto) = &mut self.proto
            && let Some(index) = HeightmapType::WORLDGEN.iter().position(|&t| t == kind)
        {
            if proto.heightmaps[index].is_none() {
                let (min_y, height) = (self.min_y, self.sections.len() as i32 * 16);
                let mut heightmap = Heightmap::new(kind, min_y, height);
                let sections = &self.sections;
                Heightmap::prime(&mut [&mut heightmap], min_y, top, |x, y, z| sections[((y - min_y) >> 4) as usize].block_state(x, (y & 15) as usize, z));
                proto.heightmaps[index] = Some(heightmap);
            }
            return proto.heightmaps[index].as_ref().expect("a primed heightmap").highest_taken((x & 15) as usize, (z & 15) as usize);
        }
        let kind = match kind {
            HeightmapType::WorldSurfaceWg => HeightmapType::WorldSurface,
            HeightmapType::OceanFloorWg => HeightmapType::OceanFloor,
            other => other,
        };
        self.height_at(kind, x, z)
    }

    pub fn mark_pos_for_post_processing(&mut self, pos: BlockPos) {
        if pos.y < self.min_y || pos.y >= self.min_y + self.height() {
            return;
        }
        let index = self.section_index(pos.y);
        self.post_processing[index].push(((pos.x & 15) | ((pos.y & 15) << 4) | ((pos.z & 15) << 8)) as u16);
    }

    pub fn initialize_light_sources(&mut self) {
        let blocks = ChunkBlocks { sections: &self.sections, min_y: self.min_y };
        self.sky_light_sources.fill_from(blocks);
    }

    pub fn update_sky_light_sources(&mut self, pos: BlockPos) {
        let blocks = ChunkBlocks { sections: &self.sections, min_y: self.min_y };
        self.sky_light_sources.update(blocks, pos.x & 15, pos.y, pos.z & 15);
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    pub fn height(&self) -> i32 {
        self.sections.len() as i32 * 16
    }

    fn section_index(&self, y: i32) -> usize {
        ((y - self.min_y) >> 4) as usize
    }

    pub fn block_state(&self, pos: BlockPos) -> BlockState {
        let i = self.section_index(pos.y);
        match self.sections.get(i) {
            Some(section) => section.block_state((pos.x & 15) as usize, (pos.y & 15) as usize, (pos.z & 15) as usize),
            None => blocks::AIR.default_state(),
        }
    }

    pub fn set_block_state(&mut self, pos: BlockPos, state: BlockState) -> Option<BlockState> {
        let i = self.section_index(pos.y);
        let section = &mut self.sections[i];
        let skipped = state.is_air();
        if section.has_only_air() && skipped {
            return None;
        }
        let (x, z) = ((pos.x & 15) as usize, (pos.z & 15) as usize);
        let old = section.set_block_state(x, (pos.y & 15) as usize, z, state);
        if old == state {
            return None;
        }
        for k in 0..self.heightmaps.len() {
            let (heightmaps, sections, min_y) = (&mut self.heightmaps, &self.sections, self.min_y);
            heightmaps[k].update(x, pos.y, z, state, |y| {
                sections[((y - min_y) >> 4) as usize].block_state(x, (y & 15) as usize, z)
            });
        }
        self.unsaved = true;
        Some(old)
    }

    pub fn prime_heightmaps(&mut self) {
        let top = self.highest_section_position() + 16;
        let (min_y, sections) = (self.min_y, &self.sections);
        let mut maps: Vec<&mut Heightmap> = self.heightmaps.iter_mut().collect();
        Heightmap::prime(&mut maps, min_y, top, |x, y, z| sections[((y - min_y) >> 4) as usize].block_state(x, (y & 15) as usize, z));
    }

    pub fn highest_section_position(&self) -> i32 {
        match self.sections.iter().rposition(|s| !s.has_only_air()) {
            Some(i) => self.min_y + i as i32 * 16,
            None => self.min_y,
        }
    }

    pub fn heightmap(&self, kind: HeightmapType) -> &Heightmap {
        &self.heightmaps[HeightmapType::LIVE.iter().position(|&t| t == kind).unwrap()]
    }

    pub fn noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        let quart_min_y = self.min_y >> 2;
        let quart_max_y = quart_min_y + ((self.sections.len() as i32 * 16) >> 2) - 1;
        let y = quart_y.clamp(quart_min_y, quart_max_y);
        self.sections[self.section_index(y << 2)].biomes.get((quart_x & 3) as usize, (y & 3) as usize, (quart_z & 3) as usize)
    }

    pub fn height_at(&self, kind: HeightmapType, x: i32, z: i32) -> i32 {
        self.heightmap(kind).highest_taken((x & 15) as usize, (z & 15) as usize)
    }

    pub fn write_data(&self, w: &mut Writer) {
        let sent: Vec<&Heightmap> = self.heightmaps.iter().filter(|h| h.kind.send_to_client()).collect();
        w.varint(sent.len() as i32);
        for heightmap in sent {
            w.varint(heightmap.kind.id());
            w.varint(heightmap.raw().len() as i32);
            heightmap.raw().iter().for_each(|&l| w.i64(l));
        }
        let mut data = Writer::new();
        for section in &self.sections {
            section.write(&mut data);
        }
        w.varint(data.as_slice().len() as i32);
        w.bytes(data.as_slice());
        let positions = self.block_entities.map.keys_in_order();
        w.varint(positions.len() as i32);
        for pos in positions {
            let entity = self.block_entities.map.get(&pos).unwrap();
            w.u8(((pos.x & 15) << 4 | (pos.z & 15)) as u8);
            w.i16(pos.y as i16);
            w.varint(entity.ty().0 as i32);
            match entity.get_update_tag() {
                Nbt::Compound(fields) if fields.is_empty() => w.u8(0),
                tag => tag.write(w),
            }
        }
    }
}

#[derive(Clone)]
pub enum ChunkGenerator {
    Flat(FlatLevelSource),
    Noise(std::sync::Arc<crate::world::level::levelgen::NoiseBasedChunkGenerator>),
}

impl From<FlatLevelSource> for ChunkGenerator {
    fn from(flat: FlatLevelSource) -> ChunkGenerator {
        ChunkGenerator::Flat(flat)
    }
}

impl ChunkGenerator {
    pub fn is_flat(&self) -> bool {
        matches!(self, ChunkGenerator::Flat(_))
    }

    pub fn generate(&self, pos: ChunkPos, dimension: &DimensionType) -> LevelChunk {
        self.generate_with_beardifier(pos, dimension, None)
    }

    pub fn generate_with_beardifier(
        &self,
        pos: ChunkPos,
        dimension: &DimensionType,
        beardifier: Option<std::sync::Arc<crate::world::level::levelgen::beardifier::Beardifier>>,
    ) -> LevelChunk {
        match self {
            ChunkGenerator::Flat(flat) => {
                let mut chunk = flat.generate(pos, dimension);
                chunk.proto = Some(Box::default());
                for kind in HeightmapType::WORLDGEN {
                    chunk.get_height(kind, pos.min_block_x(), pos.min_block_z());
                }
                chunk
            }
            ChunkGenerator::Noise(noise) => noise.generate_with_beardifier(pos, dimension, beardifier).into_level_chunk(dimension),
        }
    }

    pub fn fill_biomes_from_noise(&self, chunk: &mut LevelChunk, dimension: &DimensionType) {
        match self {
            ChunkGenerator::Flat(flat) => {
                for section in &mut chunk.sections {
                    *section = LevelChunkSection::new(flat.biome);
                }
            }
            ChunkGenerator::Noise(noise) => {
                crate::world::level::levelgen::NoiseBasedChunkGenerator::fill_biomes_from_noise(&mut chunk.sections, &noise.chunk_biomes(chunk.pos, dimension));
            }
        }
    }

    pub fn sea_level(&self) -> i32 {
        match self {
            ChunkGenerator::Flat(flat) => flat.sea_level(),
            ChunkGenerator::Noise(noise) => noise.sea_level(),
        }
    }

    pub fn origin(&self) -> ChunkPos {
        match self {
            ChunkGenerator::Flat(_) => ChunkPos::new(0, 0),
            ChunkGenerator::Noise(noise) => noise.origin(),
        }
    }

    pub fn spawn_height(&self, min_y: i32, height: i32) -> i32 {
        match self {
            ChunkGenerator::Flat(flat) => min_y + height.min(flat.layers.iter().map(|&(_, count)| count as i32).sum()),
            ChunkGenerator::Noise(_) => 64,
        }
    }
}

#[derive(Clone)]
pub struct FlatLevelSource {
    pub layers: Vec<(BlockState, u32)>,
    pub biome: Biome,
    pub structure_overrides: Option<crate::world::level::levelgen::flat::StructureOverrides>,
    pub lakes: bool,
    pub features: bool,
}

impl Default for FlatLevelSource {
    fn default() -> Self {
        FlatLevelSource {
            layers: vec![(blocks::BEDROCK.default_state(), 1), (blocks::DIRT.default_state(), 2), (blocks::GRASS_BLOCK.default_state(), 1)],
            biome: Biome::by_name("minecraft:plains").expect("plains biome"),
            structure_overrides: Some(crate::world::level::levelgen::flat::StructureOverrides::Direct(vec![
                "minecraft:strongholds".into(),
                "minecraft:villages".into(),
            ])),
            lakes: false,
            features: false,
        }
    }
}

impl FlatLevelSource {
    pub fn sea_level(&self) -> i32 {
        -63
    }

    pub fn generate(&self, pos: ChunkPos, dimension: &DimensionType) -> LevelChunk {
        let mut chunk = LevelChunk::empty(pos, dimension, self.biome);
        let mut y = chunk.min_y;
        for &(state, height) in &self.layers {
            for _ in 0..height {
                if y >= chunk.min_y + chunk.height() {
                    break;
                }
                let section = chunk.section_index(y);
                for z in 0..16 {
                    for x in 0..16 {
                        chunk.sections[section].set_block_state(x, (y & 15) as usize, z, state);
                    }
                }
                y += 1;
            }
        }
        chunk.prime_heightmaps();
        chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overworld() -> &'static DimensionType {
        &crisp_data::DIMENSION_TYPES[0]
    }

    #[test]
    fn heightmaps_follow_changes() {
        let mut chunk = FlatLevelSource::default().generate(ChunkPos::new(0, 0), overworld());
        assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 3, 3), -61);
        chunk.set_block_state(BlockPos::new(3, 10, 3), blocks::STONE.default_state());
        assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 3, 3), 10);
        chunk.set_block_state(BlockPos::new(3, 10, 3), blocks::AIR.default_state());
        assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 3, 3), -61);
    }
}
