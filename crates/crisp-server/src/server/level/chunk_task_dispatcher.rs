use std::collections::{HashMap, VecDeque};

use indexmap::IndexMap;

use crisp_core::fastutil::LongOpenHashSet;

use super::chunk_level;

pub const PRIORITY_LEVEL_COUNT: i32 = chunk_level::MAX_LEVEL + 2;

pub struct ChunkTaskPriorityQueue<T> {
    queues_per_priority: Vec<IndexMap<i64, Vec<T>>>,
    top_priority_queue_index: i32,
}

impl<T> Default for ChunkTaskPriorityQueue<T> {
    fn default() -> Self {
        ChunkTaskPriorityQueue { queues_per_priority: (0..PRIORITY_LEVEL_COUNT).map(|_| IndexMap::new()).collect(), top_priority_queue_index: PRIORITY_LEVEL_COUNT }
    }
}

impl<T> ChunkTaskPriorityQueue<T> {
    fn queue(&mut self, priority: i32) -> &mut IndexMap<i64, Vec<T>> {
        &mut self.queues_per_priority[priority as usize]
    }

    fn skip_empty_queues(&mut self) {
        while self.has_work() && self.queues_per_priority[self.top_priority_queue_index as usize].is_empty() {
            self.top_priority_queue_index += 1;
        }
    }

    pub fn resort_chunk_tasks(&mut self, old_priority: i32, pos: i64, new_priority: i32) {
        if old_priority >= PRIORITY_LEVEL_COUNT {
            return;
        }
        let old_tasks = self.queue(old_priority).shift_remove(&pos);
        if old_priority == self.top_priority_queue_index {
            self.skip_empty_queues();
        }
        if let Some(old_tasks) = old_tasks.filter(|t| !t.is_empty()) {
            self.queue(new_priority).entry(pos).or_default().extend(old_tasks);
            self.top_priority_queue_index = self.top_priority_queue_index.min(new_priority);
        }
    }

    pub fn submit(&mut self, task: T, chunk_pos: i64, level: i32) {
        self.queue(level).entry(chunk_pos).or_default().push(task);
        self.top_priority_queue_index = self.top_priority_queue_index.min(level);
    }

    pub fn release(&mut self, pos: i64, unschedule: bool) {
        for queue in &mut self.queues_per_priority {
            if let Some(tasks) = queue.get_mut(&pos) {
                if unschedule {
                    tasks.clear();
                }
                if tasks.is_empty() {
                    queue.shift_remove(&pos);
                }
            }
        }
        self.skip_empty_queues();
    }

    pub fn pop(&mut self) -> Option<(i64, Vec<T>)> {
        if !self.has_work() {
            return None;
        }
        let index = self.top_priority_queue_index;
        let popped = self.queue(index).shift_remove_index(0);
        self.skip_empty_queues();
        popped
    }

    pub fn has_work(&self) -> bool {
        self.top_priority_queue_index < PRIORITY_LEVEL_COUNT
    }
}

enum Message {
    LevelChange { pos: i64, new_level: i32 },
    Release { pos: i64, clear_queue: bool, remove_ticket: bool },
    Submit { pos: i64, level: i32 },
    Poll,
}

impl Message {
    fn priority(&self) -> usize {
        match self {
            Message::LevelChange { .. } => 0,
            Message::Release { .. } => 1,
            Message::Submit { .. } => 2,
            Message::Poll => 3,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainThreadTask {
    Run { pos: i64, last_of_batch: bool },
    RemoveTicket(i64),
    Poll,
    ReleasePlayerTicket(i64),
}

pub struct ThrottlingChunkTaskDispatcher {
    queue: ChunkTaskPriorityQueue<()>,
    sleeping: bool,
    chunk_positions_in_execution: LongOpenHashSet,
    max_chunks_in_execution: usize,
    mailbox: [VecDeque<Message>; 4],
}

impl ThrottlingChunkTaskDispatcher {
    pub fn new(max_chunks_in_execution: usize) -> Self {
        ThrottlingChunkTaskDispatcher {
            queue: ChunkTaskPriorityQueue::default(),
            sleeping: true,
            chunk_positions_in_execution: LongOpenHashSet::new(),
            max_chunks_in_execution,
            mailbox: Default::default(),
        }
    }

    fn schedule(&mut self, message: Message) {
        self.mailbox[message.priority()].push_back(message);
    }

    pub fn on_level_change(&mut self, pos: i64, new_level: i32) {
        self.schedule(Message::LevelChange { pos, new_level });
    }

    pub fn release(&mut self, pos: i64, remove_ticket: bool, clear_queue: bool) {
        self.schedule(Message::Release { pos, clear_queue, remove_ticket });
    }

    pub fn submit(&mut self, pos: i64, level: i32) {
        self.schedule(Message::Submit { pos, level });
    }

    pub fn poll_task(&mut self) {
        self.schedule(Message::Poll);
    }

    pub fn run(&mut self, queue_levels: &mut HashMap<i64, i32>, default_level: i32, out: &mut VecDeque<MainThreadTask>) {
        loop {
            let Some(message) = self.mailbox.iter_mut().find_map(|q| q.pop_front()) else { return };
            match message {
                Message::LevelChange { pos, new_level } => {
                    let old_level = queue_levels.get(&pos).copied().unwrap_or(default_level);
                    self.queue.resort_chunk_tasks(old_level, pos, new_level);
                    if new_level >= default_level {
                        queue_levels.remove(&pos);
                    } else {
                        queue_levels.insert(pos, new_level);
                    }
                }
                Message::Release { pos, clear_queue, remove_ticket } => {
                    self.queue.release(pos, clear_queue);
                    self.chunk_positions_in_execution.remove(pos);
                    if self.sleeping {
                        self.sleeping = false;
                        self.poll_task();
                    }
                    if remove_ticket {
                        out.push_back(MainThreadTask::RemoveTicket(pos));
                    }
                }
                Message::Submit { pos, level } => {
                    self.queue.submit((), pos, level);
                    if self.sleeping {
                        self.sleeping = false;
                        self.poll_task();
                    }
                }
                Message::Poll => {
                    let popped = if self.chunk_positions_in_execution.len() < self.max_chunks_in_execution { self.queue.pop() } else { None };
                    match popped {
                        None => self.sleeping = true,
                        Some((pos, tasks)) => {
                            self.chunk_positions_in_execution.add(pos);
                            if tasks.is_empty() {
                                out.push_back(MainThreadTask::Poll);
                            }
                            let count = tasks.len();
                            for i in 0..count {
                                out.push_back(MainThreadTask::Run { pos, last_of_batch: i + 1 == count });
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowest_level_first_then_in_order() {
        let mut queue = ChunkTaskPriorityQueue::default();
        queue.submit('a', 1, 5);
        queue.submit('b', 2, 3);
        queue.submit('c', 3, 5);
        queue.submit('d', 1, 5);
        queue.resort_chunk_tasks(5, 3, 2);
        assert_eq!(queue.pop(), Some((3, vec!['c'])));
        assert_eq!(queue.pop(), Some((2, vec!['b'])));
        assert_eq!(queue.pop(), Some((1, vec!['a', 'd'])));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn throttles_to_the_chunks_in_execution() {
        let mut dispatcher = ThrottlingChunkTaskDispatcher::new(2);
        let mut levels = HashMap::new();
        let mut out = VecDeque::new();
        for pos in 0..4 {
            dispatcher.submit(pos, pos as i32);
        }
        dispatcher.run(&mut levels, 34, &mut out);
        assert_eq!(out.drain(..).collect::<Vec<_>>(), [MainThreadTask::Run { pos: 0, last_of_batch: true }]);
        dispatcher.poll_task();
        dispatcher.run(&mut levels, 34, &mut out);
        assert_eq!(out.drain(..).collect::<Vec<_>>(), [MainThreadTask::Run { pos: 1, last_of_batch: true }]);
        dispatcher.poll_task();
        dispatcher.run(&mut levels, 34, &mut out);
        assert!(out.is_empty());
        dispatcher.release(0, false, false);
        dispatcher.run(&mut levels, 34, &mut out);
        assert_eq!(out.drain(..).collect::<Vec<_>>(), [MainThreadTask::Run { pos: 2, last_of_batch: true }]);
    }
}
