use crisp_core::Direction;
use crisp_data::{BlockState, blocks};

use super::{face_shape_occludes, occlusion_face};
use crate::world::level::chunk::LevelChunkSection;

#[derive(Clone, Copy)]
pub struct ChunkBlocks<'a> {
    pub sections: &'a [LevelChunkSection],
    pub min_y: i32,
}

impl ChunkBlocks<'_> {
    fn block_state(&self, x: i32, y: i32, z: i32) -> BlockState {
        let index = (y - self.min_y) >> 4;
        if index < 0 || index as usize >= self.sections.len() {
            return blocks::AIR.default_state();
        }
        self.sections[index as usize].block_state((x & 15) as usize, (y & 15) as usize, (z & 15) as usize)
    }
}

#[derive(Clone)]
pub struct ChunkSkyLightSources {
    min_y: i32,
    heightmap: Box<[i32; 256]>,
}

impl ChunkSkyLightSources {
    pub fn new(level_min_y: i32) -> ChunkSkyLightSources {
        let min_y = level_min_y - 1;
        ChunkSkyLightSources { min_y, heightmap: Box::new([min_y; 256]) }
    }

    pub fn fill_from(&mut self, chunk: ChunkBlocks) {
        let Some(max_section_index) = chunk.sections.iter().rposition(|s| !s.has_only_air()) else {
            self.heightmap.fill(self.min_y);
            return;
        };
        for z in 0..16 {
            for x in 0..16 {
                let initial_edge_y = self.find_lowest_source_y(chunk, max_section_index, x, z).max(self.min_y);
                self.heightmap[index(x, z)] = initial_edge_y;
            }
        }
    }

    fn find_lowest_source_y(&self, chunk: ChunkBlocks, top_section_index: usize, x: i32, z: i32) -> i32 {
        let mut top_y = chunk.min_y + (top_section_index as i32 + 1) * 16;
        let mut top_state = blocks::AIR.default_state();
        for section_index in (0..=top_section_index).rev() {
            let section = &chunk.sections[section_index];
            if section.has_only_air() {
                top_state = blocks::AIR.default_state();
                top_y = chunk.min_y + section_index as i32 * 16;
            } else {
                for y in (0..16).rev() {
                    let bottom_state = section.block_state(x as usize, y, z as usize);
                    if is_edge_occluded(top_state, bottom_state) {
                        return top_y;
                    }
                    top_state = bottom_state;
                    top_y -= 1;
                }
            }
        }
        self.min_y
    }

    pub fn update(&mut self, chunk: ChunkBlocks, x: i32, y: i32, z: i32) -> bool {
        let upper_edge_y = y + 1;
        let index = index(x, z);
        let current_lowest_source_y = self.heightmap[index];
        if upper_edge_y < current_lowest_source_y {
            return false;
        }
        let top_state = chunk.block_state(x, y + 1, z);
        let middle_state = chunk.block_state(x, y, z);
        if self.update_edge(chunk, index, current_lowest_source_y, (x, y + 1, z), top_state, (x, y, z), middle_state) {
            return true;
        }
        let bottom_state = chunk.block_state(x, y - 1, z);
        self.update_edge(chunk, index, current_lowest_source_y, (x, y, z), middle_state, (x, y - 1, z), bottom_state)
    }

    #[allow(clippy::too_many_arguments)]
    fn update_edge(
        &mut self,
        chunk: ChunkBlocks,
        index: usize,
        old_top_edge_y: i32,
        top_pos: (i32, i32, i32),
        top_state: BlockState,
        bottom_pos: (i32, i32, i32),
        bottom_state: BlockState,
    ) -> bool {
        let checked_edge_y = top_pos.1;
        if is_edge_occluded(top_state, bottom_state) {
            if checked_edge_y > old_top_edge_y {
                self.heightmap[index] = checked_edge_y;
                return true;
            }
        } else if checked_edge_y == old_top_edge_y {
            self.heightmap[index] = self.find_lowest_source_below(chunk, bottom_pos, bottom_state);
            return true;
        }
        false
    }

    fn find_lowest_source_below(&self, chunk: ChunkBlocks, (x, start_y, z): (i32, i32, i32), start_state: BlockState) -> i32 {
        let mut top_y = start_y;
        let mut top_state = start_state;
        let mut bottom_y = start_y - 1;
        while bottom_y >= self.min_y {
            let bottom_state = chunk.block_state(x, bottom_y, z);
            if is_edge_occluded(top_state, bottom_state) {
                return top_y;
            }
            top_state = bottom_state;
            top_y = bottom_y;
            bottom_y -= 1;
        }
        self.min_y
    }

    pub fn lowest_source_y(&self, x: i32, z: i32) -> i32 {
        self.extend_sources_below_world(self.heightmap[index(x, z)])
    }

    pub fn highest_lowest_source_y(&self) -> i32 {
        let max = self.heightmap.iter().copied().max().unwrap_or(self.min_y);
        self.extend_sources_below_world(max)
    }

    fn extend_sources_below_world(&self, value: i32) -> i32 {
        if value == self.min_y { i32::MIN } else { value }
    }
}

fn index(x: i32, z: i32) -> usize {
    (x + z * 16) as usize
}

fn is_edge_occluded(top: BlockState, bottom: BlockState) -> bool {
    if bottom.light_dampening() != 0 {
        return true;
    }
    face_shape_occludes(occlusion_face(top, Direction::Down), occlusion_face(bottom, Direction::Up))
}
