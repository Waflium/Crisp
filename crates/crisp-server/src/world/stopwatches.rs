use std::io;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Instant;

use crisp_core::fastutil::Object2ObjectOpenHashMap;
use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use crate::util::java_collections::identifier_hash;
use crate::world::level::storage::{read_dat, write_dat};

#[derive(Clone, Copy, Debug)]
pub struct Stopwatch {
    creation_time: i64,
    accumulated_elapsed_time: i64,
}

impl Stopwatch {
    pub fn new(creation_time: i64) -> Stopwatch {
        Stopwatch { creation_time, accumulated_elapsed_time: 0 }
    }

    pub fn elapsed_milliseconds(&self, current_time: i64) -> i64 {
        self.accumulated_elapsed_time + (current_time - self.creation_time)
    }

    pub fn elapsed_seconds(&self, current_time: i64) -> f64 {
        self.elapsed_milliseconds(current_time) as f64 / 1000.0
    }
}

pub struct Stopwatches {
    dir: Option<PathBuf>,
    stopwatches: Object2ObjectOpenHashMap<Stopwatch>,
    dirty: bool,
}

impl Stopwatches {
    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("stopwatches.dat")
    }

    pub fn current_time() -> i64 {
        static START: LazyLock<Instant> = LazyLock::new(Instant::now);
        START.elapsed().as_millis() as i64
    }

    pub fn load(dir: Option<PathBuf>) -> Stopwatches {
        let data = dir.as_deref().and_then(|dir| match read_dat(&Stopwatches::file(dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:stopwatches]");
                None
            }
        });
        let mut stopwatches = Object2ObjectOpenHashMap::with_hash(16, 0.75, identifier_hash);
        if let Some(Nbt::Compound(entries)) = data.as_ref().and_then(|d| d.get("stopwatches")) {
            let current_time = Stopwatches::current_time();
            for (id, elapsed) in entries {
                if let Some(elapsed) = elapsed.as_i64() {
                    stopwatches.insert(id.clone(), Stopwatch { creation_time: current_time, accumulated_elapsed_time: elapsed });
                }
            }
        }
        Stopwatches { dir, stopwatches, dirty: false }
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.stopwatches.iter().map(|(id, _)| id)
    }

    pub fn get(&self, id: &str) -> Option<&Stopwatch> {
        self.stopwatches.get(id)
    }

    pub fn add(&mut self, id: &str, stopwatch: Stopwatch) -> bool {
        if self.stopwatches.contains_key(id) {
            return false;
        }
        self.stopwatches.insert(id.to_string(), stopwatch);
        self.dirty = true;
        true
    }

    pub fn update(&mut self, id: &str, update: impl FnOnce(Stopwatch) -> Stopwatch) -> bool {
        let Some(stopwatch) = self.stopwatches.get_mut(id) else { return false };
        *stopwatch = update(*stopwatch);
        self.dirty = true;
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let removed = self.stopwatches.remove(id).is_some();
        self.dirty |= removed;
        removed
    }

    fn is_dirty(&self) -> bool {
        self.dirty || self.stopwatches.len() > 0
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.is_dirty() {
            return Ok(());
        }
        let current_time = Stopwatches::current_time();
        let entries = self.stopwatches.iter().map(|(id, s)| (id.to_string(), Nbt::Long(s.elapsed_milliseconds(current_time)))).collect();
        let data = Nbt::Compound(vec![("stopwatches".into(), Nbt::Compound(entries))]);
        write_dat(&Stopwatches::file(dir), &Nbt::Compound(vec![("data".into(), data), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]))?;
        self.dirty = false;
        Ok(())
    }
}
