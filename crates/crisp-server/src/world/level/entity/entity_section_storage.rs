use std::collections::BTreeSet;

use crisp_core::fastutil::{Long2ObjectOpenHashMap, LongOpenHashSet};
use crisp_core::{AABB, ChunkPos, SectionPos, mth};

use super::entity_section::EntitySection;
use super::visibility::Visibility;

#[derive(Default)]
pub struct EntitySectionStorage {
    sections: Long2ObjectOpenHashMap<EntitySection>,
    section_ids: BTreeSet<i64>,
}

fn pos_to_section_coord(pos: f64) -> i32 {
    mth::floor(pos) >> 4
}

fn section_x(key: i64) -> i32 {
    (key >> 42) as i32
}

fn section_y(key: i64) -> i32 {
    (key << 44 >> 44) as i32
}

fn section_z(key: i64) -> i32 {
    (key << 22 >> 42) as i32
}

pub fn chunk_key_from_section_key(key: i64) -> i64 {
    ChunkPos::new(section_x(key), section_z(key)).pack()
}

impl EntitySectionStorage {
    pub fn count(&self) -> usize {
        self.section_ids.len()
    }

    pub fn for_each_accessible_non_empty_section(&self, bb: &AABB, mut output: impl FnMut(&EntitySection)) {
        let (x_min, y_min, z_min) = (pos_to_section_coord(bb.min_x - 2.0), pos_to_section_coord(bb.min_y - 4.0), pos_to_section_coord(bb.min_z - 2.0));
        let (x_max, y_max, z_max) = (pos_to_section_coord(bb.max_x + 2.0), pos_to_section_coord(bb.max_y + 0.0), pos_to_section_coord(bb.max_z + 2.0));
        for x in x_min..=x_max {
            let lowest = SectionPos::as_long(x, 0, 0);
            let highest = SectionPos::as_long(x, -1, -1);
            for &key in self.section_ids.range(lowest..=highest) {
                let (y, z) = (section_y(key), section_z(key));
                if y >= y_min
                    && y <= y_max
                    && z >= z_min
                    && z <= z_max
                    && let Some(section) = self.sections.get(key)
                    && !section.is_empty()
                    && section.status().is_accessible()
                {
                    output(section);
                }
            }
        }
    }

    pub fn existing_section_positions_in_chunk(&self, chunk_key: i64) -> impl Iterator<Item = i64> + '_ {
        let pos = ChunkPos::unpack(chunk_key);
        self.section_ids.range(SectionPos::as_long(pos.x, 0, pos.z)..=SectionPos::as_long(pos.x, -1, pos.z)).copied()
    }

    pub fn get_or_create_section(&mut self, key: i64, initial_visibility: impl FnOnce(i64) -> Visibility) -> &mut EntitySection {
        if !self.sections.contains_key(key) {
            let status = initial_visibility(chunk_key_from_section_key(key));
            self.section_ids.insert(key);
            self.sections.insert(key, EntitySection::new(status));
        }
        self.sections.get_mut(key).unwrap()
    }

    pub fn get_section(&self, key: i64) -> Option<&EntitySection> {
        self.sections.get(key)
    }

    pub fn get_section_mut(&mut self, key: i64) -> Option<&mut EntitySection> {
        self.sections.get_mut(key)
    }

    pub fn all_chunks_with_existing_sections(&self) -> LongOpenHashSet {
        let mut chunks = LongOpenHashSet::new();
        for key in self.sections.keys() {
            chunks.add(chunk_key_from_section_key(key));
        }
        chunks
    }

    pub fn remove(&mut self, key: i64) {
        self.sections.remove(key);
        self.section_ids.remove(&key);
    }
}
