use crisp_core::hash_map::FastMap;

use crisp_core::ChunkPos;
use crisp_core::fastutil::LongLinkedOpenHashSet;
use crisp_core::mth;

pub const SOURCE: i64 = (1875066i64 & 0xFFFF_FFFF) | ((1875066i64 & 0xFFFF_FFFF) << 32);

pub trait TrackerLevels {
    fn level(&self, node: i64) -> i32;
    fn set_level(&mut self, node: i64, level: i32);
    fn level_from_source(&self, node: i64) -> i32;
}

struct LeveledPriorityQueue {
    level_count: i32,
    queues: Vec<LongLinkedOpenHashSet>,
    first_queued_level: i32,
}

impl LeveledPriorityQueue {
    fn new(level_count: i32) -> LeveledPriorityQueue {
        LeveledPriorityQueue { level_count, queues: (0..level_count).map(|_| LongLinkedOpenHashSet::new()).collect(), first_queued_level: level_count }
    }

    fn remove_first(&mut self) -> i64 {
        let queue = &mut self.queues[self.first_queued_level as usize];
        let result = queue.remove_first().expect("queued node");
        if queue.is_empty() {
            self.check_first_queued_level(self.level_count);
        }
        result
    }

    fn is_empty(&self) -> bool {
        self.first_queued_level >= self.level_count
    }

    fn dequeue(&mut self, node: i64, key: i32, upper_bound: i32) {
        let queue = &mut self.queues[key as usize];
        queue.remove(node);
        if queue.is_empty() && self.first_queued_level == key {
            self.check_first_queued_level(upper_bound);
        }
    }

    fn enqueue(&mut self, node: i64, key: i32) {
        self.queues[key as usize].add(node);
        if self.first_queued_level > key {
            self.first_queued_level = key;
        }
    }

    fn check_first_queued_level(&mut self, upper_bound: i32) {
        let old_level = self.first_queued_level;
        self.first_queued_level = upper_bound;
        for i in old_level + 1..upper_bound {
            if !self.queues[i as usize].is_empty() {
                self.first_queued_level = i;
                break;
            }
        }
    }
}

pub struct ChunkTracker {
    pub level_count: i32,
    queue: LeveledPriorityQueue,
    computed_levels: FastMap<i64, u8>,
}

const NO_COMPUTED_LEVEL: i32 = 255;

impl ChunkTracker {
    pub fn new(level_count: i32) -> ChunkTracker {
        assert!(level_count < 254, "Level count must be < 254.");
        ChunkTracker { level_count, queue: LeveledPriorityQueue::new(level_count), computed_levels: FastMap::default() }
    }

    fn computed_level(&self, node: i64) -> i32 {
        self.computed_levels.get(&node).map_or(NO_COMPUTED_LEVEL, |&l| l as i32)
    }

    fn calculate_priority(&self, level: i32, computed_level: i32) -> i32 {
        level.min(computed_level).min(self.level_count - 1)
    }

    pub fn has_work(&self) -> bool {
        !self.queue.is_empty()
    }

    pub fn update(&mut self, levels: &mut impl TrackerLevels, node: i64, new_level_from: i32, only_decreased: bool) {
        self.check_edge(levels, SOURCE, node, new_level_from, only_decreased);
    }

    fn check_edge(&mut self, levels: &mut impl TrackerLevels, from: i64, to: i64, new_level_from: i32, only_decreased: bool) {
        let level_to = levels.level(to);
        let computed = self.computed_level(to);
        self.check_edge_with(levels, from, to, new_level_from, level_to, computed, only_decreased);
    }

    #[allow(clippy::too_many_arguments)]
    fn check_edge_with(&mut self, levels: &mut impl TrackerLevels, from: i64, to: i64, new_level_from: i32, level_to: i32, old_computed_level: i32, only_decreased: bool) {
        if to == SOURCE {
            return;
        }
        let max = self.level_count - 1;
        let new_level_from = mth::clamp_i(new_level_from, 0, max);
        let level_to = mth::clamp_i(level_to, 0, max);
        let was_consistent = old_computed_level == NO_COMPUTED_LEVEL;
        let old_computed_level = if was_consistent { level_to } else { old_computed_level };
        let new_computed_level =
            if only_decreased { old_computed_level.min(new_level_from) } else { mth::clamp_i(self.get_computed_level(levels, to, from, new_level_from), 0, max) };
        let old_priority = self.calculate_priority(level_to, old_computed_level);
        if level_to != new_computed_level {
            let new_priority = self.calculate_priority(level_to, new_computed_level);
            if old_priority != new_priority && !was_consistent {
                self.queue.dequeue(to, old_priority, new_priority);
            }
            self.queue.enqueue(to, new_priority);
            self.computed_levels.insert(to, new_computed_level as u8);
        } else if !was_consistent {
            self.queue.dequeue(to, old_priority, self.level_count);
            self.computed_levels.remove(&to);
        }
    }

    fn check_neighbor(&mut self, levels: &mut impl TrackerLevels, from: i64, to: i64, level: i32, only_decreased: bool) {
        let stored_old_computed_level = self.computed_level(to);
        let level_from = mth::clamp_i(self.compute_level_from_neighbor(levels, from, to, level), 0, self.level_count - 1);
        if only_decreased {
            let level_to = levels.level(to);
            self.check_edge_with(levels, from, to, level_from, level_to, stored_old_computed_level, only_decreased);
        } else {
            let was_consistent = stored_old_computed_level == NO_COMPUTED_LEVEL;
            let old_computed_level =
                if was_consistent { mth::clamp_i(levels.level(to), 0, self.level_count - 1) } else { stored_old_computed_level };
            if level_from == old_computed_level {
                let level_to = if was_consistent { old_computed_level } else { levels.level(to) };
                self.check_edge_with(levels, from, to, self.level_count - 1, level_to, stored_old_computed_level, only_decreased);
            }
        }
    }

    pub fn run_updates(&mut self, levels: &mut impl TrackerLevels, mut count: i32) -> i32 {
        if self.queue.is_empty() {
            return count;
        }
        while !self.queue.is_empty() && count > 0 {
            count -= 1;
            let node = self.queue.remove_first();
            let level = mth::clamp_i(levels.level(node), 0, self.level_count - 1);
            let computed_level = self.computed_levels.remove(&node).map_or(NO_COMPUTED_LEVEL, |l| l as i32);
            if computed_level < level {
                levels.set_level(node, computed_level);
                self.check_neighbors_after_update(levels, node, computed_level, true);
            } else if computed_level > level {
                levels.set_level(node, self.level_count - 1);
                if computed_level != self.level_count - 1 {
                    self.queue.enqueue(node, self.calculate_priority(self.level_count - 1, computed_level));
                    self.computed_levels.insert(node, computed_level as u8);
                }
                self.check_neighbors_after_update(levels, node, level, false);
            }
        }
        count
    }

    fn check_neighbors_after_update(&mut self, levels: &mut impl TrackerLevels, node: i64, level: i32, only_decrease: bool) {
        if only_decrease && level >= self.level_count - 2 {
            return;
        }
        let pos = ChunkPos::unpack(node);
        for offset_x in -1..=1 {
            for offset_z in -1..=1 {
                let neighbor = ChunkPos::new(pos.x + offset_x, pos.z + offset_z).pack();
                if neighbor != node {
                    self.check_neighbor(levels, node, neighbor, level, only_decrease);
                }
            }
        }
    }

    fn get_computed_level(&self, levels: &impl TrackerLevels, node: i64, known_parent: i64, known_level_from_parent: i32) -> i32 {
        let mut computed_level = known_level_from_parent;
        let pos = ChunkPos::unpack(node);
        for offset_x in -1..=1 {
            for offset_z in -1..=1 {
                let mut neighbor = ChunkPos::new(pos.x + offset_x, pos.z + offset_z).pack();
                if neighbor == node {
                    neighbor = SOURCE;
                }
                if neighbor != known_parent {
                    let cost = self.compute_level_from_neighbor(levels, neighbor, node, levels.level(neighbor));
                    if computed_level > cost {
                        computed_level = cost;
                    }
                    if computed_level == 0 {
                        return computed_level;
                    }
                }
            }
        }
        computed_level
    }

    fn compute_level_from_neighbor(&self, levels: &impl TrackerLevels, from: i64, to: i64, from_level: i32) -> i32 {
        if from == SOURCE { levels.level_from_source(to) } else { from_level + 1 }
    }
}
