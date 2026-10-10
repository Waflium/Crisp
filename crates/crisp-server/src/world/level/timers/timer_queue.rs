use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;
use indexmap::IndexMap;

use super::TimerCallback;
use crate::server::Server;
use crate::world::level::storage::{read_dat, write_dat};

struct Event {
    trigger_time: i64,
    id: String,
    callback: TimerCallback,
}

pub struct TimerQueue {
    dir: Option<PathBuf>,
    queue: BTreeMap<(i64, u64), Event>,
    sequential_id: u64,
    events: IndexMap<String, IndexMap<i64, (i64, u64)>>,
    dirty: bool,
}

impl TimerQueue {
    fn file(dir: &Path) -> PathBuf {
        dir.join("minecraft").join("scheduled_events.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> TimerQueue {
        let mut queue = TimerQueue { dir, queue: BTreeMap::new(), sequential_id: 0, events: IndexMap::new(), dirty: false };
        let Some(dir) = queue.dir.clone() else { return queue };
        let data = match read_dat(&TimerQueue::file(&dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:scheduled_events]");
                None
            }
        };
        let Some(data) = data else {
            queue.dirty = true;
            return queue;
        };
        let parsed: Option<Vec<(i64, String, TimerCallback)>> = match data.get("events") {
            Some(Nbt::List(events)) => events
                .iter()
                .map(|event| {
                    let trigger_time = event.get("trigger_time").and_then(Nbt::as_i64)?;
                    let id = event.get("id").and_then(Nbt::as_str)?.to_string();
                    Some((trigger_time, id, TimerCallback::parse(event.get("callback")?)?))
                })
                .collect(),
            _ => None,
        };
        match parsed {
            Some(events) => events.into_iter().for_each(|(time, id, callback)| queue.schedule(&id, time, callback)),
            None => {
                error!("Error loading saved data: SavedDataType[minecraft:scheduled_events]");
                queue.dirty = true;
            }
        }
        queue
    }

    pub fn tick(server: &mut Server, current_tick: i64) {
        loop {
            let queue = &mut server.scheduled_events;
            let Some(entry) = queue.queue.first_entry() else { return };
            if entry.get().trigger_time > current_tick {
                return;
            }
            let event = entry.remove();
            queue.remove_cell(&event.id, current_tick);
            queue.dirty = true;
            event.callback.handle(server);
        }
    }

    fn remove_cell(&mut self, id: &str, time: i64) {
        if let Some(row) = self.events.get_mut(id) {
            row.shift_remove(&time);
            if row.is_empty() {
                self.events.shift_remove(id);
            }
        }
    }

    pub fn schedule(&mut self, id: &str, time: i64, callback: TimerCallback) {
        if self.events.get(id).is_some_and(|row| row.contains_key(&time)) {
            return;
        }
        self.sequential_id += 1;
        let key = (time, self.sequential_id);
        self.events.entry(id.to_string()).or_default().insert(time, key);
        self.queue.insert(key, Event { trigger_time: time, id: id.to_string(), callback });
        self.dirty = true;
    }

    pub fn remove(&mut self, id: &str) -> i32 {
        let removed = self.events.shift_remove(id).unwrap_or_default();
        for key in removed.values() {
            self.queue.remove(key);
        }
        self.dirty = true;
        removed.len() as i32
    }

    pub fn events_ids(&self) -> impl Iterator<Item = &str> {
        self.events.keys().map(String::as_str)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let events = self
            .queue
            .values()
            .map(|event| {
                Nbt::Compound(vec![
                    ("trigger_time".into(), Nbt::Long(event.trigger_time)),
                    ("callback".into(), event.callback.to_nbt()),
                    ("id".into(), Nbt::String(event.id.clone())),
                ])
            })
            .collect();
        let data = Nbt::Compound(vec![("events".into(), Nbt::List(events))]);
        let file = Nbt::Compound(vec![("data".into(), data), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        write_dat(&TimerQueue::file(dir), &file)?;
        self.dirty = false;
        Ok(())
    }
}
