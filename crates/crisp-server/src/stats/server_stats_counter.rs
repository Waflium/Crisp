use std::path::PathBuf;

use crisp_core::fastutil::Object2ObjectOpenHashMap;
use crisp_core::random::java_string_hash;
use crisp_data::WORLD_VERSION;
use crisp_data::packets::play::clientbound;
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};

use super::stat::Stat;
use super::stat_type::StatType;
use super::stats_counter::StatsCounter;
use crate::network::connection::Outbound;
use crate::util::file_util;
use crate::util::java_collections::JavaHashMap;

const DEFAULT_TABLE_SIZE: usize = 16;

pub struct ServerStatsCounter {
    stats: IndexMap<Stat, i32>,
    presized: usize,
    file: PathBuf,
    dirty: IndexSet<Stat>,
    dirty_table_size: usize,
}

impl Default for ServerStatsCounter {
    fn default() -> ServerStatsCounter {
        ServerStatsCounter { stats: IndexMap::new(), presized: 0, file: PathBuf::new(), dirty: IndexSet::new(), dirty_table_size: DEFAULT_TABLE_SIZE }
    }
}

impl StatsCounter for ServerStatsCounter {
    fn stats(&self) -> &IndexMap<Stat, i32> {
        &self.stats
    }

    fn set_value(&mut self, stat: Stat, count: i32) {
        self.stats.insert(stat, count);
        self.mark_dirty(stat);
    }
}

impl ServerStatsCounter {
    pub fn new(file: PathBuf) -> ServerStatsCounter {
        let mut counter = ServerStatsCounter { file, ..ServerStatsCounter::default() };
        if counter.file.is_file() {
            match std::fs::read(&counter.file) {
                Err(e) => error!("Couldn't read statistics file {}\n{e}", counter.file.display()),
                Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                    Err(e) => error!("Couldn't parse statistics file {}\ncom.google.gson.JsonSyntaxException: {e}", counter.file.display()),
                    Ok(element) => counter.parse(&element),
                },
            }
        }
        counter
    }

    pub fn save(&self) {
        let result = file_util::create_directories_safe(self.file.parent().unwrap_or(&self.file))
            .and_then(|_| std::fs::write(&self.file, serde_json::to_string_pretty(&self.to_json()).expect("stats serialize")));
        if let Err(e) = result {
            error!("Couldn't save stats to {}\n{e}", self.file.display());
        }
    }

    fn mark_dirty(&mut self, stat: Stat) {
        if self.dirty.insert(stat) && self.dirty.len() > self.dirty_table_size * 3 / 4 {
            self.dirty_table_size *= 2;
        }
    }

    fn get_dirty(&mut self) -> Vec<Stat> {
        let dirty = hash_order(self.dirty.iter().copied(), self.dirty_table_size);
        let result = hash_order(dirty, table_size_for((self.dirty.len().max(12) as f64 / 0.75).ceil() as usize));
        self.dirty.clear();
        result
    }

    pub fn parse(&mut self, element: &Value) {
        let mut errors = Vec::new();
        let mut grouped: Vec<Vec<(Stat, i32)>> = Vec::new();
        match element.get("stats") {
            None => {}
            Some(Value::Object(types)) => {
                for (type_key, values) in types {
                    let stat_type = registry_key(type_key, "minecraft:stat_type", |id| StatType::by_key(id));
                    let entries = match values {
                        Value::Object(entries) => Ok(entries),
                        other => Err(format!("Not a JSON object: {other}")),
                    };
                    let (stat_type, entries) = match (stat_type, entries) {
                        (Ok(stat_type), Ok(entries)) => (stat_type, entries),
                        (stat_type, entries) => {
                            errors.extend(stat_type.err());
                            errors.extend(entries.err());
                            continue;
                        }
                    };
                    let mut group = Vec::new();
                    for (key, value) in entries {
                        let stat = registry_key(key, stat_type.registry_key(), |id| {
                            stat_type.get_registry().iter().position(|&e| e == id).map(|value| stat_type.get(value as u16))
                        });
                        match (stat, int_value(value)) {
                            (Ok(stat), Ok(count)) => group.push((stat, count)),
                            (stat, count) => {
                                errors.extend(stat.err());
                                errors.extend(count.err());
                            }
                        }
                    }
                    grouped.push(group);
                }
            }
            Some(other) => errors.push(format!("Not a JSON object: {other}")),
        }
        if !errors.is_empty() {
            error!("Failed to parse statistics for {}: {}", self.file.display(), errors.join("; "));
        }
        let parsed = parsed_stats_order(&grouped);
        self.presized = self.stats.len() + parsed.len();
        self.stats.extend(parsed);
    }

    fn open_hash_map(&self) -> Object2ObjectOpenHashMap<Stat> {
        let mut map = Object2ObjectOpenHashMap::new(DEFAULT_TABLE_SIZE, 0.75);
        map.try_capacity(self.presized);
        for &stat in self.stats.keys() {
            map.insert(stat.get_name().to_string(), stat);
        }
        map
    }

    fn to_json(&self) -> Value {
        let mut grouped: Vec<(StatType, JavaHashMap<Stat>)> = Vec::new();
        for (_, &stat) in self.open_hash_map().stream() {
            let index = match grouped.iter().position(|(stat_type, _)| *stat_type == stat.stat_type) {
                Some(index) => index,
                None => {
                    grouped.push((stat.stat_type, JavaHashMap::new(java_string_hash)));
                    grouped.len() - 1
                }
            };
            grouped[index].1.insert(stat.get_name().to_string(), stat);
        }
        grouped.sort_by_key(|(stat_type, _)| stat_type.id());
        let mut stats = Map::new();
        for (stat_type, values) in grouped {
            let entries: Map<String, Value> = values.iter().map(|(_, &stat)| (stat.value_key().to_string(), Value::from(self.get_value(stat)))).collect();
            stats.insert(stat_type.key().to_string(), Value::Object(entries));
        }
        let mut result = Map::new();
        result.insert("stats".into(), Value::Object(stats));
        result.insert("DataVersion".into(), Value::from(WORLD_VERSION));
        Value::Object(result)
    }

    pub fn mark_all_dirty(&mut self) {
        let keys: Vec<Stat> = self.open_hash_map().iter().map(|(_, &stat)| stat).collect();
        for stat in keys {
            self.mark_dirty(stat);
        }
    }

    pub fn send_stats(&mut self, connection: &Outbound) {
        let mut stats_to_send = Object2ObjectOpenHashMap::new(DEFAULT_TABLE_SIZE, 0.75);
        for stat in self.get_dirty() {
            stats_to_send.insert(stat.get_name().to_string(), (stat, self.get_value(stat)));
        }
        connection.send(clientbound::AWARD_STATS, |w| {
            w.varint(stats_to_send.len() as i32);
            for (_, &(stat, value)) in stats_to_send.iter() {
                stat.write(w);
                w.varint(value);
            }
        });
    }
}

fn registry_key<T>(key: &str, registry: &str, lookup: impl FnOnce(&str) -> Option<T>) -> Result<T, String> {
    let id = crate::resources::identifier::read(key)?;
    lookup(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {id}"))
}

fn int_value(value: &Value) -> Result<i32, String> {
    match value {
        Value::Number(number) => Ok(match number.as_i64() {
            Some(v) => v as i32,
            None => number.as_f64().map_or(0, |v| v as i64 as i32),
        }),
        Value::Bool(b) => Ok(*b as i32),
        other => Err(format!("Not a number: {other}")),
    }
}

fn hash_bucket(stat: Stat, table_size: usize) -> usize {
    let h = java_string_hash(stat.get_name());
    (h ^ ((h as u32) >> 16) as i32) as u32 as usize & (table_size - 1)
}

fn hash_order(stats: impl IntoIterator<Item = Stat>, table_size: usize) -> Vec<Stat> {
    let mut order: Vec<Stat> = stats.into_iter().collect();
    order.sort_by_key(|&stat| hash_bucket(stat, table_size));
    order
}

fn table_size_for(capacity: usize) -> usize {
    capacity.next_power_of_two()
}

fn parsed_stats_order(grouped: &[Vec<(Stat, i32)>]) -> Vec<(Stat, i32)> {
    let (mut table_size, mut threshold, mut size) = (0usize, 0usize, 0usize);
    for group in grouped.iter().filter(|group| !group.is_empty()) {
        if table_size == 0 {
            let t = (group.len() as f64 / 0.75).ceil() as usize;
            if t > threshold {
                threshold = table_size_for(t);
            }
        } else {
            while group.len() > threshold {
                table_size *= 2;
                threshold = table_size * 3 / 4;
            }
        }
        for _ in group {
            if table_size == 0 {
                table_size = if threshold > 0 { threshold } else { DEFAULT_TABLE_SIZE };
                threshold = table_size * 3 / 4;
            }
            size += 1;
            if size > threshold {
                table_size *= 2;
                threshold = table_size * 3 / 4;
            }
        }
    }
    let mut order: Vec<(Stat, i32)> = grouped.iter().flatten().copied().collect();
    order.sort_by_key(|&(stat, _)| hash_bucket(stat, table_size.max(1)));
    order
}
