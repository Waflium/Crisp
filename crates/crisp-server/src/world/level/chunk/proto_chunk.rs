use crisp_core::ChunkPos;
use crisp_data::{Block, BlockState, DimensionType, Fluid, blocks};

use super::{Biome, Heightmap, HeightmapType, LevelChunk, LevelChunkSection};
use crate::world::ticks::ProtoChunkTicks;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ChunkStatus {
    Empty,
    StructureStarts,
    StructureReferences,
    Biomes,
    Terrain,
    Features,
    InitializeLight,
    Light,
    Spawn,
    Full,
}

impl ChunkStatus {
    pub fn heightmaps_after(self) -> &'static [HeightmapType] {
        if self < ChunkStatus::Terrain { &HeightmapType::WORLDGEN } else { &HeightmapType::LIVE }
    }
}

pub struct ProtoChunkData {
    pub heightmaps: [Option<Heightmap>; 2],
    pub block_ticks: ProtoChunkTicks<Block>,
    pub fluid_ticks: ProtoChunkTicks<Fluid>,
}

impl Default for ProtoChunkData {
    fn default() -> ProtoChunkData {
        ProtoChunkData { heightmaps: [None, None], block_ticks: ProtoChunkTicks::default(), fluid_ticks: ProtoChunkTicks::default() }
    }
}

pub struct ProtoChunk {
    pub pos: ChunkPos,
    pub min_y: i32,
    pub sections: Vec<LevelChunkSection>,
    pub status: ChunkStatus,
    heightmaps: [Option<Heightmap>; 6],
    pub post_processing: Vec<Vec<u16>>,
}

impl ProtoChunk {
    pub fn new(pos: ChunkPos, dimension: &DimensionType) -> ProtoChunk {
        let count = dimension.height as usize / 16;
        let plains = Biome::by_name("minecraft:plains").expect("plains");
        ProtoChunk {
            pos,
            min_y: dimension.min_y,
            sections: (0..count).map(|_| LevelChunkSection::new(plains)).collect(),
            status: ChunkStatus::Empty,
            heightmaps: Default::default(),
            post_processing: vec![Vec::new(); count],
        }
    }

    pub fn height(&self) -> i32 {
        self.sections.len() as i32 * 16
    }

    pub fn max_y(&self) -> i32 {
        self.min_y + self.height() - 1
    }

    pub fn section_index(&self, y: i32) -> usize {
        ((y - self.min_y) >> 4) as usize
    }

    pub fn is_outside_build_height(&self, y: i32) -> bool {
        y < self.min_y || y > self.max_y()
    }

    pub fn block_state(&self, x: i32, y: i32, z: i32) -> BlockState {
        if self.is_outside_build_height(y) {
            return blocks::VOID_AIR.default_state();
        }
        self.sections[self.section_index(y)].block_state((x & 15) as usize, (y & 15) as usize, (z & 15) as usize)
    }

    pub fn heightmap_mut(&mut self, kind: HeightmapType) -> &mut Heightmap {
        let (min_y, height) = (self.min_y, self.height());
        self.heightmaps[kind.id() as usize].get_or_insert_with(|| Heightmap::new(kind, min_y, height))
    }

    pub fn heightmap(&self, kind: HeightmapType) -> Option<&Heightmap> {
        self.heightmaps[kind.id() as usize].as_ref()
    }

    pub fn height_at(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32 {
        if self.heightmap(kind).is_none() {
            self.prime_heightmaps(&[kind]);
        }
        self.heightmap(kind).unwrap().highest_taken((x & 15) as usize, (z & 15) as usize)
    }

    pub fn set_block_state(&mut self, x: i32, y: i32, z: i32, state: BlockState) -> BlockState {
        if self.is_outside_build_height(y) {
            return blocks::VOID_AIR.default_state();
        }
        let index = self.section_index(y);
        let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
        let section = &mut self.sections[index];
        if section.has_only_air() && state.is_block(blocks::AIR) {
            return state;
        }
        let old = section.set_block_state(lx, (y & 15) as usize, lz, state);
        let mut to_prime = Vec::new();
        for &kind in self.status.heightmaps_after() {
            let (heightmaps, sections, min_y) = (&mut self.heightmaps, &self.sections, self.min_y);
            match &mut heightmaps[kind.id() as usize] {
                Some(heightmap) => {
                    heightmap.update(lx, y, lz, state, |y| sections[((y - min_y) >> 4) as usize].block_state(lx, (y & 15) as usize, lz));
                }
                None => to_prime.push(kind),
            }
        }
        if !to_prime.is_empty() {
            self.prime_heightmaps(&to_prime);
        }
        old
    }

    pub fn update_heightmap(&mut self, kind: HeightmapType, x: i32, y: i32, z: i32, state: BlockState) {
        let (lx, lz) = ((x & 15) as usize, (z & 15) as usize);
        let (min_y, height) = (self.min_y, self.height());
        let heightmap = self.heightmaps[kind.id() as usize].get_or_insert_with(|| Heightmap::new(kind, min_y, height));
        let sections = &self.sections;
        heightmap.update(lx, y, lz, state, |y| sections[((y - min_y) >> 4) as usize].block_state(lx, (y & 15) as usize, lz));
    }

    pub fn mark_pos_for_post_processing(&mut self, x: i32, y: i32, z: i32) {
        if self.is_outside_build_height(y) {
            return;
        }
        let packed = ((x & 15) | ((y & 15) << 4) | ((z & 15) << 8)) as u16;
        let index = self.section_index(y);
        self.post_processing[index].push(packed);
    }

    pub fn highest_filled_section_index(&self) -> Option<usize> {
        self.sections.iter().rposition(|s| !s.has_only_air())
    }

    pub fn prime_heightmaps(&mut self, kinds: &[HeightmapType]) {
        let top = match self.highest_filled_section_index() {
            Some(i) => self.min_y + i as i32 * 16 + 16,
            None => self.min_y + 16,
        };
        let (min_y, height) = (self.min_y, self.height());
        for &kind in kinds {
            self.heightmaps[kind.id() as usize].get_or_insert_with(|| Heightmap::new(kind, min_y, height));
        }
        let sections = &self.sections;
        let mut maps: Vec<&mut Heightmap> = self.heightmaps.iter_mut().flatten().filter(|h| kinds.contains(&h.kind)).collect();
        Heightmap::prime(&mut maps, min_y, top, |x, y, z| sections[((y - min_y) >> 4) as usize].block_state(x, (y & 15) as usize, z));
    }

    pub fn noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        let quart_min_y = self.min_y >> 2;
        let quart_max_y = quart_min_y + (self.height() >> 2) - 1;
        let y = quart_y.clamp(quart_min_y, quart_max_y);
        let section = self.section_index(y << 2);
        self.sections[section].biomes.get((quart_x & 3) as usize, (y & 3) as usize, (quart_z & 3) as usize)
    }

    pub fn into_level_chunk(mut self, dimension: &DimensionType) -> LevelChunk {
        let mut heightmaps = Vec::new();
        for kind in HeightmapType::LIVE {
            heightmaps.push(self.heightmaps[kind.id() as usize].take());
        }
        let proto = ProtoChunkData { heightmaps: HeightmapType::WORLDGEN.map(|kind| self.heightmaps[kind.id() as usize].take()), ..ProtoChunkData::default() };
        let mut chunk = LevelChunk::with_sections(self.pos, dimension, self.sections);
        chunk.proto = Some(Box::new(proto));
        chunk.post_processing = self.post_processing;
        let mut missing = false;
        for (i, heightmap) in heightmaps.into_iter().enumerate() {
            match heightmap {
                Some(h) => chunk.heightmaps[i] = h,
                None => missing = true,
            }
        }
        if missing {
            chunk.prime_heightmaps();
        }
        chunk.unsaved = true;
        chunk
    }
}
