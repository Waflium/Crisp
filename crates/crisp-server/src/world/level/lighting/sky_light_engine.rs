use crisp_core::{ChunkPos, Direction};
use crisp_data::blocks;

use super::light_engine::{LightEngine, PULL_LIGHT_IN_ENTRY, Propagator, opacity, queue_entry, state_at};
use super::sky_light_sources::ChunkSkyLightSources;
use super::{LightChunkGetter, is_empty_shape, node, section, shape_occludes};
use crate::world::level::light_layer::LightLayer;

const REMOVE_TOP_SKY_SOURCE_ENTRY: i64 = queue_entry::decrease_all_directions(15);
const REMOVE_SKY_SOURCE_ENTRY: i64 = queue_entry::decrease_skip_one_direction(15, Direction::Up);
const ADD_SKY_SOURCE_ENTRY: i64 = queue_entry::increase_skip_one_direction(15, false, Direction::Up);

pub struct SkyLightEngine {
    pub base: LightEngine,
    empty_chunk_sources: ChunkSkyLightSources,
}

fn is_source_level(value: u8) -> bool {
    value == 15
}

impl SkyLightEngine {
    pub fn new(level_min_y: i32) -> SkyLightEngine {
        SkyLightEngine { base: LightEngine::new(LightLayer::Sky), empty_chunk_sources: ChunkSkyLightSources::new(level_min_y) }
    }

    fn chunk_sources<C: LightChunkGetter + ?Sized>(chunks: &C, chunk_x: i32, chunk_z: i32) -> Option<&ChunkSkyLightSources> {
        chunks.chunk_for_lighting(chunk_x, chunk_z).map(|c| &c.sky_light_sources)
    }

    fn lowest_source_y<C: LightChunkGetter + ?Sized>(chunks: &C, x: i32, z: i32, default_value: i32) -> i32 {
        match Self::chunk_sources(chunks, x >> 4, z >> 4) {
            Some(sources) => sources.lowest_source_y(x & 15, z & 15),
            None => default_value,
        }
    }

    fn update_sources_in_column<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, x: i32, z: i32, lowest_source_y: i32) {
        let world_bottom_y = self.base.storage.bottom_section_y().wrapping_shl(4);
        self.remove_sources_below(x, z, lowest_source_y, world_bottom_y);
        self.add_sources_above(chunks, x, z, lowest_source_y, world_bottom_y);
    }

    fn remove_sources_below(&mut self, x: i32, z: i32, lowest_source_y: i32, world_bottom_y: i32) {
        if lowest_source_y <= world_bottom_y {
            return;
        }
        let (section_x, section_z) = (x >> 4, z >> 4);
        let start_y = lowest_source_y - 1;
        let mut section_y = start_y >> 4;
        while self.base.storage.has_light_data_at_or_below(section_y) {
            if self.base.storage.storing_light_for_section(section::of(section_x, section_y, section_z)) {
                let section_bottom_y = section_y << 4;
                let section_top_y = section_bottom_y + 15;
                let mut y = section_top_y.min(start_y);
                while y >= section_bottom_y {
                    let block_node = node::of(x, y, z);
                    if !is_source_level(self.base.storage.stored_level(block_node)) {
                        return;
                    }
                    self.base.storage.set_stored_level(block_node, 0);
                    let data = if y == lowest_source_y - 1 { REMOVE_TOP_SKY_SOURCE_ENTRY } else { REMOVE_SKY_SOURCE_ENTRY };
                    self.base.enqueue_decrease(block_node, data);
                    y -= 1;
                }
            }
            section_y -= 1;
        }
    }

    fn add_sources_above<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, x: i32, z: i32, lowest_source_y: i32, world_bottom_y: i32) {
        let (section_x, section_z) = (x >> 4, z >> 4);
        let neighbor_lowest_source_y = Self::lowest_source_y(chunks, x - 1, z, i32::MIN)
            .max(Self::lowest_source_y(chunks, x + 1, z, i32::MIN))
            .max(Self::lowest_source_y(chunks, x, z - 1, i32::MIN).max(Self::lowest_source_y(chunks, x, z + 1, i32::MIN)));
        let start_y = lowest_source_y.max(world_bottom_y);
        let mut section_node = section::of(section_x, start_y >> 4, section_z);
        while !self.base.storage.is_above_data(section_node) {
            if self.base.storage.storing_light_for_section(section_node) {
                let section_bottom_y = section::y(section_node) << 4;
                let section_top_y = section_bottom_y + 15;
                for y in section_bottom_y.max(start_y)..=section_top_y {
                    let block_node = node::of(x, y, z);
                    if is_source_level(self.base.storage.stored_level(block_node)) {
                        return;
                    }
                    self.base.storage.set_stored_level(block_node, 15);
                    if y < neighbor_lowest_source_y || y == lowest_source_y {
                        self.base.enqueue_increase(block_node, ADD_SKY_SOURCE_ENTRY);
                    }
                }
            }
            section_node = section::offset(section_node, 0, 1, 0);
        }
    }

    fn count_empty_sections_below_if_at_border(&self, block_node: i64) -> i32 {
        let y = node::y(block_node);
        if y & 15 != 0 {
            return 0;
        }
        let (x, z) = (node::x(block_node), node::z(block_node));
        let (local_x, local_z) = (x & 15, z & 15);
        if local_x != 0 && local_x != 15 && local_z != 0 && local_z != 15 {
            return 0;
        }
        let (section_x, section_y, section_z) = (x >> 4, y >> 4, z >> 4);
        let storage = &self.base.storage;
        let mut empty_sections_below = 0;
        while !storage.storing_light_for_section(section::of(section_x, section_y - empty_sections_below - 1, section_z))
            && storage.has_light_data_at_or_below(section_y - empty_sections_below - 1)
        {
            empty_sections_below += 1;
        }
        empty_sections_below
    }

    fn propagate_from_empty_sections(&mut self, to_node: i64, direction: Direction, to_level: i32, increase: bool, empty_sections_below: i32) {
        if empty_sections_below == 0 {
            return;
        }
        let (x, z) = (node::x(to_node), node::z(to_node));
        if !crossed_section_edge(direction, x & 15, z & 15) {
            return;
        }
        let y = node::y(to_node);
        let (section_x, section_z) = (x >> 4, z >> 4);
        let mut section_y = (y >> 4) - 1;
        let bottom_section_y = section_y - empty_sections_below + 1;
        while section_y >= bottom_section_y {
            if !self.base.storage.storing_light_for_section(section::of(section_x, section_y, section_z)) {
                section_y -= 1;
                continue;
            }
            let section_min_y = section_y << 4;
            for local_y in (0..16).rev() {
                let block_node = node::of(x, section_min_y + local_y, z);
                if increase {
                    self.base.storage.set_stored_level(block_node, to_level as u8);
                    if to_level > 1 {
                        self.base.enqueue_increase(block_node, queue_entry::increase_skip_one_direction(to_level, true, direction.opposite()));
                    }
                } else {
                    self.base.storage.set_stored_level(block_node, 0);
                    self.base.enqueue_decrease(block_node, queue_entry::decrease_skip_one_direction(to_level, direction.opposite()));
                }
            }
            section_y -= 1;
        }
    }

    pub fn set_light_enabled<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, pos: ChunkPos, enable: bool) {
        self.base.set_light_enabled(pos, enable);
        if !enable {
            return;
        }
        let sources = Self::chunk_sources(chunks, pos.x, pos.z).unwrap_or(&self.empty_chunk_sources);
        let highest_non_source_y = sources.highest_lowest_source_y().wrapping_sub(1);
        let lowest_fully_source_section_y = (highest_non_source_y >> 4) + 1;
        let zero_node = section::zero_node(pos.x, pos.z);
        let top_section_y = self.base.storage.top_section_y(zero_node);
        let bottom_section_y = self.base.storage.bottom_section_y().max(lowest_fully_source_section_y);
        let mut section_y = top_section_y.wrapping_sub(1);
        while section_y >= bottom_section_y {
            if let Some(layer) = self.base.storage.data_layer_to_write(section::of(pos.x, section_y, pos.z))
                && layer.is_empty()
            {
                layer.fill(15);
            }
            section_y -= 1;
        }
    }

    pub fn propagate_light_sources<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, pos: ChunkPos) {
        let zero_node = section::zero_node(pos.x, pos.z);
        self.base.storage.set_light_enabled(zero_node, true);
        let empty = &self.empty_chunk_sources;
        let sources = Self::chunk_sources(chunks, pos.x, pos.z).unwrap_or(empty);
        let north_sources = Self::chunk_sources(chunks, pos.x, pos.z - 1).unwrap_or(empty);
        let south_sources = Self::chunk_sources(chunks, pos.x, pos.z + 1).unwrap_or(empty);
        let west_sources = Self::chunk_sources(chunks, pos.x - 1, pos.z).unwrap_or(empty);
        let east_sources = Self::chunk_sources(chunks, pos.x + 1, pos.z).unwrap_or(empty);
        let top_section_y = self.base.storage.top_section_y(zero_node);
        let bottom_section_y = self.base.storage.bottom_section_y();
        let (section_min_x, section_min_z) = (pos.min_block_x(), pos.min_block_z());
        let mut increases = Vec::new();
        let mut section_y = top_section_y.wrapping_sub(1);
        while section_y >= bottom_section_y {
            let section_node = section::of(pos.x, section_y, pos.z);
            let base = &mut self.base;
            if let Some(data_layer) = base.storage.data_layer_to_write(section_node) {
                let section_min_y = section_y << 4;
                let section_max_y = section_min_y + 15;
                let mut sources_below = false;
                for z in 0..16 {
                    for x in 0..16 {
                        let lowest_source_y = sources.lowest_source_y(x, z);
                        if lowest_source_y > section_max_y {
                            continue;
                        }
                        let north = if z == 0 { north_sources.lowest_source_y(x, 15) } else { sources.lowest_source_y(x, z - 1) };
                        let south = if z == 15 { south_sources.lowest_source_y(x, 0) } else { sources.lowest_source_y(x, z + 1) };
                        let west = if x == 0 { west_sources.lowest_source_y(15, z) } else { sources.lowest_source_y(x - 1, z) };
                        let east = if x == 15 { east_sources.lowest_source_y(0, z) } else { sources.lowest_source_y(x + 1, z) };
                        let neighbor_lowest_source_y = north.max(south).max(west.max(east));
                        let mut y = section_max_y;
                        while y >= section_min_y.max(lowest_source_y) {
                            data_layer.set(x, y & 15, z, 15);
                            if y == lowest_source_y || y < neighbor_lowest_source_y {
                                let block_node = node::of(section_min_x + x, y, section_min_z + z);
                                let data =
                                    queue_entry::increase_sky_source_in_directions(y == lowest_source_y, y < north, y < south, y < west, y < east);
                                increases.push((block_node, data));
                            }
                            y -= 1;
                        }
                        if lowest_source_y < section_min_y {
                            sources_below = true;
                        }
                    }
                }
                for (block_node, data) in increases.drain(..) {
                    base.enqueue_increase(block_node, data);
                }
                if !sources_below {
                    break;
                }
            }
            section_y -= 1;
        }
    }
}

fn crossed_section_edge(direction: Direction, x: i32, z: i32) -> bool {
    match direction {
        Direction::North => z == 15,
        Direction::South => z == 0,
        Direction::West => x == 15,
        Direction::East => x == 0,
        _ => false,
    }
}

impl Propagator for SkyLightEngine {
    fn base(&mut self) -> &mut LightEngine {
        &mut self.base
    }

    fn check_node<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, block_node: i64) {
        let (x, y, z) = (node::x(block_node), node::y(block_node), node::z(block_node));
        let section_node = node::to_section(block_node);
        let lowest_source_y =
            if self.base.storage.light_on_in_section(section_node) { Self::lowest_source_y(chunks, x, z, i32::MAX) } else { i32::MAX };
        if lowest_source_y != i32::MAX {
            self.update_sources_in_column(chunks, x, z, lowest_source_y);
        }
        if !self.base.storage.storing_light_for_section(section_node) {
            return;
        }
        if y >= lowest_source_y {
            self.base.enqueue_decrease(block_node, REMOVE_SKY_SOURCE_ENTRY);
            self.base.enqueue_increase(block_node, ADD_SKY_SOURCE_ENTRY);
        } else {
            let old_level = self.base.storage.stored_level(block_node) as i32;
            if old_level > 0 {
                self.base.storage.set_stored_level(block_node, 0);
                self.base.enqueue_decrease(block_node, queue_entry::decrease_all_directions(old_level));
            } else {
                self.base.enqueue_decrease(block_node, PULL_LIGHT_IN_ENTRY);
            }
        }
    }

    fn propagate_increase<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, from_node: i64, increase_data: i64, from_level: i32) {
        let mut from_state = None;
        let empty_sections_below = self.count_empty_sections_below_if_at_border(from_node);
        for direction in Direction::VALUES {
            if !queue_entry::should_propagate_in_direction(increase_data, direction) {
                continue;
            }
            let to_node = node::offset(from_node, direction);
            if !self.base.storage.storing_light_for_section(node::to_section(to_node)) {
                continue;
            }
            let to_level = self.base.storage.stored_level(to_node) as i32;
            if from_level - 1 <= to_level {
                continue;
            }
            let to_state = state_at(chunks, to_node);
            let new_to_level = from_level - opacity(to_state);
            if new_to_level <= to_level {
                continue;
            }
            let from_state = *from_state.get_or_insert_with(|| {
                if queue_entry::is_from_empty_shape(increase_data) { blocks::AIR.default_state() } else { state_at(chunks, from_node) }
            });
            if !shape_occludes(from_state, to_state, direction) {
                self.base.storage.set_stored_level(to_node, new_to_level as u8);
                if new_to_level > 1 {
                    let data = queue_entry::increase_skip_one_direction(new_to_level, is_empty_shape(to_state), direction.opposite());
                    self.base.enqueue_increase(to_node, data);
                }
                self.propagate_from_empty_sections(to_node, direction, new_to_level, true, empty_sections_below);
            }
        }
    }

    fn propagate_decrease<C: LightChunkGetter + ?Sized>(&mut self, _chunks: &C, from_node: i64, decrease_data: i64) {
        let empty_sections_below = self.count_empty_sections_below_if_at_border(from_node);
        let old_from_level = queue_entry::from_level(decrease_data);
        for direction in Direction::VALUES {
            if !queue_entry::should_propagate_in_direction(decrease_data, direction) {
                continue;
            }
            let to_node = node::offset(from_node, direction);
            if !self.base.storage.storing_light_for_section(node::to_section(to_node)) {
                continue;
            }
            let to_level = self.base.storage.stored_level(to_node) as i32;
            if to_level == 0 {
                continue;
            }
            if to_level < old_from_level {
                self.base.storage.set_stored_level(to_node, 0);
                self.base.enqueue_decrease(to_node, queue_entry::decrease_skip_one_direction(to_level, direction.opposite()));
                self.propagate_from_empty_sections(to_node, direction, to_level, false, empty_sections_below);
            } else {
                self.base.enqueue_increase(to_node, queue_entry::increase_only_one_direction(to_level, false, direction.opposite()));
            }
        }
    }
}
