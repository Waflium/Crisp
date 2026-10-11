use crisp_core::hash_map::FastMap;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::hash::Hash;

use crisp_core::{BlockPos, ChunkPos};

use crate::world::level::levelgen::structure::BoundingBox;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct TickPriority(pub i32);

impl TickPriority {
    pub const EXTREMELY_HIGH: TickPriority = TickPriority(-3);
    pub const VERY_HIGH: TickPriority = TickPriority(-2);
    pub const HIGH: TickPriority = TickPriority(-1);
    pub const NORMAL: TickPriority = TickPriority(0);
    pub const LOW: TickPriority = TickPriority(1);
    pub const VERY_LOW: TickPriority = TickPriority(2);
    pub const EXTREMELY_LOW: TickPriority = TickPriority(3);

    pub fn by_value(value: i32) -> TickPriority {
        TickPriority(value.clamp(-3, 3))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScheduledTick<T> {
    pub ty: T,
    pub pos: BlockPos,
    pub trigger_tick: i64,
    pub priority: TickPriority,
    pub sub_tick_order: i64,
}

impl<T> ScheduledTick<T> {
    fn drain_key(&self) -> (i64, TickPriority, i64) {
        (self.trigger_tick, self.priority, self.sub_tick_order)
    }

    fn intra_tick_key(&self) -> (TickPriority, i64) {
        (self.priority, self.sub_tick_order)
    }
}

impl<T> PartialEq for ScheduledTick<T> {
    fn eq(&self, other: &Self) -> bool {
        self.drain_key() == other.drain_key()
    }
}

impl<T> Eq for ScheduledTick<T> {}

impl<T> PartialOrd for ScheduledTick<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for ScheduledTick<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.drain_key().cmp(&other.drain_key())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedTick<T> {
    pub ty: T,
    pub pos: BlockPos,
    pub delay: i32,
    pub priority: TickPriority,
}

pub struct LevelChunkTicks<T> {
    queue: BTreeSet<ScheduledTick<T>>,
    per_position: HashSet<(BlockPos, T)>,
    pending: Option<Vec<SavedTick<T>>>,
}

impl<T: Copy + Eq + Hash> Default for LevelChunkTicks<T> {
    fn default() -> Self {
        LevelChunkTicks { queue: BTreeSet::new(), per_position: HashSet::new(), pending: None }
    }
}

impl<T: Copy + Eq + Hash> LevelChunkTicks<T> {
    pub fn count(&self) -> usize {
        self.queue.len() + self.pending.as_ref().map_or(0, Vec::len)
    }

    pub fn with_pending(pending: Vec<SavedTick<T>>) -> LevelChunkTicks<T> {
        let per_position = pending.iter().map(|t| (t.pos, t.ty)).collect();
        LevelChunkTicks { queue: BTreeSet::new(), per_position, pending: Some(pending) }
    }

    fn peek(&self) -> Option<&ScheduledTick<T>> {
        self.queue.first()
    }

    fn poll(&mut self) -> Option<ScheduledTick<T>> {
        let tick = self.queue.pop_first()?;
        self.per_position.remove(&(tick.pos, tick.ty));
        Some(tick)
    }

    fn schedule(&mut self, tick: ScheduledTick<T>) {
        if self.per_position.insert((tick.pos, tick.ty)) {
            self.queue.insert(tick);
        }
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, ty: T) -> bool {
        self.per_position.contains(&(pos, ty))
    }

    pub fn pack(&self, current_tick: i64) -> Vec<SavedTick<T>> {
        let mut out: Vec<SavedTick<T>> = self.pending.clone().unwrap_or_default();
        let mut sorted: Vec<&ScheduledTick<T>> = self.queue.iter().collect();
        sorted.sort_by_key(|t| t.sub_tick_order);
        out.extend(sorted.into_iter().map(|t| SavedTick { ty: t.ty, pos: t.pos, delay: (t.trigger_tick - current_tick) as i32, priority: t.priority }));
        out
    }

    fn unpack(&mut self, current_tick: i64) {
        if let Some(pending) = self.pending.take() {
            let mut sub_tick = -(pending.len() as i64);
            for t in pending {
                self.queue.insert(ScheduledTick { ty: t.ty, pos: t.pos, trigger_tick: current_tick + t.delay as i64, priority: t.priority, sub_tick_order: sub_tick });
                sub_tick += 1;
            }
        }
    }
}

pub struct ProtoChunkTicks<T> {
    ticks: Vec<SavedTick<T>>,
    ticks_per_position: HashSet<(BlockPos, T)>,
}

impl<T: Copy + Eq + Hash> Default for ProtoChunkTicks<T> {
    fn default() -> Self {
        ProtoChunkTicks { ticks: Vec::new(), ticks_per_position: HashSet::new() }
    }
}

impl<T: Copy + Eq + Hash> ProtoChunkTicks<T> {
    pub fn schedule(&mut self, ty: T, pos: BlockPos, priority: TickPriority) {
        if self.ticks_per_position.insert((pos, ty)) {
            self.ticks.push(SavedTick { ty, pos, delay: 0, priority });
        }
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, ty: T) -> bool {
        self.ticks_per_position.contains(&(pos, ty))
    }

    pub fn scheduled_ticks(self) -> Vec<SavedTick<T>> {
        self.ticks
    }

    pub fn pack(&self) -> &[SavedTick<T>] {
        &self.ticks
    }

    pub fn load(ticks: Vec<SavedTick<T>>) -> ProtoChunkTicks<T> {
        let mut result = ProtoChunkTicks::default();
        for tick in ticks {
            if result.ticks_per_position.insert((tick.pos, tick.ty)) {
                result.ticks.push(tick);
            }
        }
        result
    }
}

pub struct LevelTicks<T> {
    containers: FastMap<ChunkPos, LevelChunkTicks<T>>,
    to_run_this_tick: HashSet<(BlockPos, T)>,
}

impl<T: Copy + Eq + Hash> Default for LevelTicks<T> {
    fn default() -> Self {
        LevelTicks { containers: FastMap::default(), to_run_this_tick: HashSet::new() }
    }
}

impl<T: Copy + Eq + Hash> LevelTicks<T> {
    pub fn count(&self) -> usize {
        self.containers.values().map(LevelChunkTicks::count).sum()
    }

    pub fn add_container(&mut self, pos: ChunkPos, mut container: LevelChunkTicks<T>, current_tick: i64) {
        container.unpack(current_tick);
        self.containers.insert(pos, container);
    }

    pub fn register_container(&mut self, pos: ChunkPos, container: LevelChunkTicks<T>) {
        self.containers.insert(pos, container);
    }

    pub fn unpack_container(&mut self, pos: ChunkPos, current_tick: i64) {
        if let Some(container) = self.containers.get_mut(&pos) {
            container.unpack(current_tick);
        }
    }

    pub fn pack(&self, pos: ChunkPos, current_tick: i64) -> Vec<SavedTick<T>> {
        self.containers.get(&pos).map_or_else(Vec::new, |c| c.pack(current_tick))
    }

    pub fn remove_container(&mut self, pos: ChunkPos) -> Option<LevelChunkTicks<T>> {
        self.containers.remove(&pos)
    }

    pub fn container(&self, pos: ChunkPos) -> Option<&LevelChunkTicks<T>> {
        self.containers.get(&pos)
    }

    pub fn schedule(&mut self, tick: ScheduledTick<T>) {
        self.containers.entry(tick.pos.chunk()).or_default().schedule(tick);
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, ty: T) -> bool {
        self.containers.get(&pos.chunk()).is_some_and(|c| c.has_scheduled_tick(pos, ty))
    }

    pub fn will_tick_this_tick(&self, pos: BlockPos, ty: T) -> bool {
        self.to_run_this_tick.contains(&(pos, ty))
    }

    pub fn collect(&mut self, current_tick: i64, max_ticks: usize, can_tick: impl Fn(ChunkPos) -> bool) -> Vec<ScheduledTick<T>> {
        let mut due: Vec<ChunkPos> = self
            .containers
            .iter()
            .filter(|(pos, c)| c.peek().is_some_and(|t| t.trigger_tick <= current_tick) && can_tick(**pos))
            .map(|(pos, _)| *pos)
            .collect();
        let mut out = Vec::new();
        while out.len() < max_ticks && !due.is_empty() {
            let best = (0..due.len()).min_by_key(|&i| self.containers[&due[i]].peek().unwrap().intra_tick_key()).unwrap();
            let pos = due.swap_remove(best);
            let next_best = due.iter().map(|p| self.containers[p].peek().unwrap().intra_tick_key()).min();
            let container = self.containers.get_mut(&pos).unwrap();
            out.push(container.poll().unwrap());
            while out.len() < max_ticks {
                match container.peek() {
                    Some(t) if t.trigger_tick <= current_tick && next_best.is_none_or(|n| t.intra_tick_key() <= n) => out.push(container.poll().unwrap()),
                    _ => break,
                }
            }
            if container.peek().is_some_and(|t| t.trigger_tick <= current_tick) && out.len() < max_ticks {
                due.push(pos);
            }
        }
        self.to_run_this_tick = out.iter().map(|t| (t.pos, t.ty)).collect();
        out
    }

    pub fn copy_area(&mut self, area: &BoundingBox, offset: BlockPos) {
        let mut ticks: Vec<ScheduledTick<T>> = Vec::new();
        for x in area.min.x >> 4..=area.max.x >> 4 {
            for z in area.min.z >> 4..=area.max.z >> 4 {
                if let Some(container) = self.containers.get(&ChunkPos::new(x, z)) {
                    ticks.extend(container.queue.iter().filter(|t| area.is_inside(t.pos)).copied());
                }
            }
        }
        let (Some(min), Some(max)) = (ticks.iter().map(|t| t.sub_tick_order).min(), ticks.iter().map(|t| t.sub_tick_order).max()) else { return };
        for tick in ticks {
            let pos = tick.pos.offset(offset.x, offset.y, offset.z);
            self.schedule(ScheduledTick { pos, sub_tick_order: tick.sub_tick_order - min + max + 1, ..tick });
        }
    }

    pub fn ran(&mut self, pos: BlockPos, ty: T) {
        self.to_run_this_tick.remove(&(pos, ty));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(pos: BlockPos, trigger: i64, priority: TickPriority, sub: i64) -> ScheduledTick<u16> {
        ScheduledTick { ty: 1, pos, trigger_tick: trigger, priority, sub_tick_order: sub }
    }

    #[test]
    fn drains_by_priority_then_schedule_order() {
        let mut ticks = LevelTicks::default();
        ticks.add_container(ChunkPos::new(0, 0), LevelChunkTicks::default(), 0);
        ticks.add_container(ChunkPos::new(1, 0), LevelChunkTicks::default(), 0);
        let a = BlockPos::new(0, 0, 0);
        let b = BlockPos::new(1, 0, 0);
        let c = BlockPos::new(16, 0, 0);
        ticks.schedule(tick(a, 5, TickPriority::NORMAL, 3));
        ticks.schedule(tick(b, 5, TickPriority::HIGH, 4));
        ticks.schedule(tick(c, 5, TickPriority::NORMAL, 1));
        ticks.schedule(tick(c, 9, TickPriority::NORMAL, 7));
        assert!(ticks.collect(4, 100, |_| true).is_empty());
        let order: Vec<BlockPos> = ticks.collect(5, 100, |_| true).iter().map(|t| t.pos).collect();
        assert_eq!(order, [b, c, a]);
    }
}
