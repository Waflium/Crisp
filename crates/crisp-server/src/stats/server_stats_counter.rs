use std::path::PathBuf;

use crisp_core::fastutil::Object2ObjectOpenHashMap;
use crisp_core::random::java_string_hash;
use crisp_data::WORLD_VERSION;
use crisp_data::packets::play::clientbound;
use serde_json::{Map, Value};

use super::stat::Stat;
use super::stat_type::StatType;
use super::stats_counter::StatsCounter;
use crate::network::connection::Outbound;
use crate::util::{FastIndexMap, FastIndexSet, file_util};
use crate::util::java_collections::JavaHashMap;

const DEFAULT_TABLE_SIZE: usize = 16;

pub struct ServerStatsCounter {
    stats: FastIndexMap<Stat, i32>,
    presized: usize,
    file: PathBuf,
    dirty: FastIndexSet<Stat>,
    dirty_table_size: usize,
}

impl Default for ServerStatsCounter {
    fn default() -> ServerStatsCounter {
        ServerStatsCounter { stats: FastIndexMap::default(), presized: 0, file: PathBuf::new(), dirty: FastIndexSet::default(), dirty_table_size: DEFAULT_TABLE_SIZE }
    }
}

impl StatsCounter for ServerStatsCounter {
    fn stats(&self) -> &FastIndexMap<Stat, i32> {
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
        let mut grouped: Vec<Vec<(Stat, i32)>> = Vec::new();
        let error = match element.get("stats") {
            None => None,
            Some(Value::Object(types)) => {
                let mut errors = Vec::new();
                let mut failed = Map::new();
                for (type_key, values) in types {
                    let stat_type = registry_key(type_key, "minecraft:stat_type", StatType::by_key);
                    let typed = match &stat_type {
                        Ok(stat_type) => parse_typed_stats(*stat_type, values),
                        Err(message) => Err(message.clone()),
                    };
                    let entry_errors: Vec<&str> = [typed.as_ref().map_or_else(|e| Some(e.as_str()), |t| t.1.as_deref()), stat_type.as_ref().err().map(String::as_str)]
                        .into_iter()
                        .flatten()
                        .collect();
                    if !entry_errors.is_empty() {
                        errors.push(entry_errors.join("; "));
                        failed.insert(type_key.clone(), values.clone());
                    }
                    if let (Ok(_), Ok((group, _))) = (&stat_type, typed) {
                        grouped.push(group);
                    }
                }
                partial_error(errors, failed)
            }
            Some(other) => Some(format!("Not a JSON object: {other}")),
        };
        if let Some(error) = error {
            error!("Failed to parse statistics for {}: {error}", self.file.display());
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

    fn stats_to_send(&mut self) -> Object2ObjectOpenHashMap<(Stat, i32)> {
        let mut stats_to_send = Object2ObjectOpenHashMap::new(DEFAULT_TABLE_SIZE, 0.75);
        for stat in self.get_dirty() {
            stats_to_send.insert(stat.get_name().to_string(), (stat, self.get_value(stat)));
        }
        stats_to_send
    }

    pub fn send_stats(&mut self, connection: &Outbound) {
        let stats_to_send = self.stats_to_send();
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

fn parse_typed_stats(stat_type: StatType, values: &Value) -> Result<(Vec<(Stat, i32)>, Option<String>), String> {
    let Value::Object(entries) = values else { return Err(format!("Not a JSON object: {values}")) };
    let mut group = Vec::new();
    let mut errors = Vec::new();
    let mut failed = Map::new();
    for (key, value) in entries {
        let stat = registry_key(key, stat_type.registry_key(), |id| stat_type.get_registry().iter().position(|&e| e == id).map(|value| stat_type.get(value as u16)));
        match (stat, int_value(value)) {
            (Ok(stat), Ok(count)) => group.push((stat, count)),
            (stat, count) => {
                let entry_errors: Vec<String> = [count.err(), stat.err()].into_iter().flatten().collect();
                errors.push(entry_errors.join("; "));
                failed.insert(key.clone(), value.clone());
            }
        }
    }
    Ok((group, partial_error(errors, failed)))
}

fn partial_error(errors: Vec<String>, failed: Map<String, Value>) -> Option<String> {
    if errors.is_empty() {
        return None;
    }
    let messages: Vec<String> = errors.into_iter().rev().collect();
    Some(format!("{} missed input: {}", messages.join("; "), Value::Object(failed)))
}

fn int_value(value: &Value) -> Result<i32, String> {
    match value {
        Value::Number(number) => Ok(match (number.as_i64(), number.as_u64()) {
            (Some(v), _) => v as i32,
            (None, Some(v)) => v as i32,
            (None, None) => number.as_f64().map_or(0, |v| v.trunc() as i128 as i32),
        }),
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(i64);

    impl Lcg {
        fn next(&mut self, bound: i32) -> i32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (((self.0 as u64) >> 33) % bound as u64) as i32
        }

        fn stat(&mut self) -> Stat {
            let stat_type = StatType::VALUES[self.next(9) as usize];
            let spread = stat_type.get_registry().len().min(60) as i32;
            stat_type.get(self.next(spread) as u16)
        }
    }

    fn send(counter: &mut ServerStatsCounter, out: &mut String) {
        out.push_str(" S");
        for (_, &(stat, value)) in counter.stats_to_send().iter() {
            out.push_str(&format!(" {}={value}", stat.get_name()));
        }
    }

    fn json(counter: &ServerStatsCounter, out: &mut String) -> Value {
        let json = counter.to_json();
        for (key, inner) in json["stats"].as_object().unwrap() {
            out.push_str(&format!(" J {key}"));
            for (stat, value) in inner.as_object().unwrap() {
                out.push_str(&format!(" {stat}={value}"));
            }
        }
        json
    }

    #[test]
    #[ignore]
    fn matches_vanilla_order_oracle() {
        let expected = std::fs::read_to_string(std::env::var("STATS_ORACLE").expect("STATS_ORACLE")).unwrap();
        for (trial, line) in expected.lines().enumerate() {
            let mut random = Lcg(trial as i64);
            let mut out = format!("T{trial}");
            let mut counter = ServerStatsCounter::default();
            let ops = 1 + random.next(150);
            for _ in 0..ops {
                match random.next(12) {
                    0 => send(&mut counter, &mut out),
                    1 => {
                        counter.mark_all_dirty();
                        out.push_str(" M");
                    }
                    _ => {
                        let stat = random.stat();
                        let count = 1 + random.next(5);
                        counter.increment(stat, count);
                    }
                }
            }
            send(&mut counter, &mut out);
            let saved = json(&counter, &mut out);
            let mut reloaded = ServerStatsCounter::default();
            reloaded.parse(&saved);
            out.push_str(" R");
            reloaded.mark_all_dirty();
            send(&mut reloaded, &mut out);
            for _ in 0..random.next(20) {
                let stat = random.stat();
                let count = 1 + random.next(5);
                reloaded.increment(stat, count);
            }
            send(&mut reloaded, &mut out);
            json(&reloaded, &mut out);
            assert_eq!(out, line, "trial {trial}");
        }
    }
}
