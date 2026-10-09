use crisp_core::hash_map::FastMap;

use crate::world::entity::EntityId;

#[derive(Default)]
pub struct LinkedIds {
    order: Vec<EntityId>,
    index: FastMap<EntityId, usize>,
    iterating: bool,
}

const HOLE: EntityId = 0;

impl LinkedIds {
    pub fn add(&mut self, id: EntityId) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.index.entry(id) {
            entry.insert(self.order.len());
            self.order.push(id);
        }
    }

    pub fn remove(&mut self, id: EntityId) {
        if let Some(i) = self.index.remove(&id) {
            self.order[i] = HOLE;
            if !self.iterating && self.order.len() >= 32 && self.index.len() * 2 < self.order.len() {
                self.compact();
            }
        }
    }

    fn compact(&mut self) {
        self.order.retain(|&id| id != HOLE);
        for (i, &id) in self.order.iter().enumerate() {
            self.index.insert(id, i);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.order.iter().copied().filter(|&id| id != HOLE)
    }

    pub fn begin_iteration(&mut self) -> usize {
        self.iterating = true;
        self.order.len()
    }

    pub fn at(&self, i: usize) -> Option<EntityId> {
        Some(self.order[i]).filter(|&id| id != HOLE)
    }

    pub fn end_iteration(&mut self) {
        self.iterating = false;
        if self.order.len() >= 32 && self.index.len() * 2 < self.order.len() {
            self.compact();
        }
    }
}

pub type EntityTickList = LinkedIds;

#[cfg(test)]
mod tests {
    use super::LinkedIds;

    #[test]
    fn keeps_insertion_order_through_removals() {
        let mut ids = LinkedIds::default();
        for id in 1..=100 {
            ids.add(id);
        }
        for id in (1..=100).filter(|id| id % 3 != 0) {
            ids.remove(id);
        }
        ids.add(3);
        ids.add(7);
        let expected: Vec<i32> = (1..=100).filter(|id| id % 3 == 0).chain([7]).collect();
        assert_eq!(ids.iter().collect::<Vec<_>>(), expected);

        let end = ids.begin_iteration();
        ids.add(1000);
        ids.remove(6);
        let seen: Vec<i32> = (0..end).filter_map(|i| ids.at(i)).collect();
        ids.end_iteration();
        assert_eq!(seen, expected.iter().copied().filter(|&id| id != 6).collect::<Vec<_>>());
        assert_eq!(ids.iter().last(), Some(1000));
    }
}
