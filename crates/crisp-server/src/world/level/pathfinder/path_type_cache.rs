use crisp_core::BlockPos;
use crisp_core::fastutil::mix;

use super::path_type::PathType;
use super::walk_node_evaluator::get_path_type_from_state;
use crate::world::level::path_navigation_region::PathNavigationRegion;

const SIZE: usize = 4096;
const MASK: i64 = 4095;

pub struct PathTypeCache {
    positions: Box<[i64; SIZE]>,
    path_types: Box<[Option<PathType>; SIZE]>,
}

impl Default for PathTypeCache {
    fn default() -> PathTypeCache {
        PathTypeCache { positions: Box::new([0; SIZE]), path_types: Box::new([None; SIZE]) }
    }
}

impl PathTypeCache {
    pub fn get_or_compute(&mut self, level: &PathNavigationRegion, pos: BlockPos) -> PathType {
        let key = pos.as_long();
        let index = index(key);
        match self.get(index, key) {
            Some(cached_path_type) => cached_path_type,
            None => self.compute(level, pos, index, key),
        }
    }

    fn get(&self, index: usize, key: i64) -> Option<PathType> {
        if self.positions[index] == key { self.path_types[index] } else { None }
    }

    fn compute(&mut self, level: &PathNavigationRegion, pos: BlockPos, index: usize, key: i64) -> PathType {
        let path_type = get_path_type_from_state(level, pos);
        self.positions[index] = key;
        self.path_types[index] = Some(path_type);
        path_type
    }

    pub fn invalidate(&mut self, pos: BlockPos) {
        let key = pos.as_long();
        let index = index(key);
        if self.positions[index] == key {
            self.path_types[index] = None;
        }
    }
}

fn index(pos: i64) -> usize {
    (mix(pos) & MASK) as usize
}
