use std::io;
use std::path::PathBuf;

use crisp_core::fastutil::Object2ObjectOpenHashMap;
use crisp_core::random::{XoroshiroRandomSource, mix_stafford13, seed_from_hash_of};
use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use crate::util::java_collections::identifier_hash;
use crate::world::level::storage::{read_dat, write_dat};

fn create_sequence(seed: i64, key: Option<&str>) -> XoroshiroRandomSource {
    let mut lo = seed ^ 7640891576956012809;
    let mut hi = lo.wrapping_add(-7046029254386353131);
    if let Some(key) = key {
        let (key_lo, key_hi) = seed_from_hash_of(key);
        lo ^= key_lo;
        hi ^= key_hi;
    }
    XoroshiroRandomSource::from_parts(mix_stafford13(lo), mix_stafford13(hi))
}

pub struct RandomSequences {
    dir: Option<PathBuf>,
    salt: i32,
    include_world_seed: bool,
    include_sequence_id: bool,
    sequences: Object2ObjectOpenHashMap<XoroshiroRandomSource>,
    dirty: bool,
}

impl RandomSequences {
    fn empty(dir: Option<PathBuf>) -> RandomSequences {
        RandomSequences {
            dir,
            salt: 0,
            include_world_seed: true,
            include_sequence_id: true,
            sequences: Object2ObjectOpenHashMap::with_hash(16, 0.75, identifier_hash),
            dirty: false,
        }
    }

    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("random_sequences.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> RandomSequences {
        let data = dir.as_deref().and_then(|dir| match read_dat(&RandomSequences::file(dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:random_sequences]");
                None
            }
        });
        let mut sequences = RandomSequences::empty(dir);
        let Some(data) = data else { return sequences };
        let Some(salt) = data.get("salt").and_then(Nbt::as_i64) else { return sequences };
        let flag = |key: &str| data.get(key).and_then(Nbt::as_i64).is_none_or(|v| v != 0);
        sequences.salt = salt as i32;
        sequences.include_world_seed = flag("include_world_seed");
        sequences.include_sequence_id = flag("include_sequence_id");
        if let Some(Nbt::Compound(entries)) = data.get("sequences") {
            for (key, sequence) in entries {
                let state = match sequence.get("source") {
                    Some(Nbt::LongArray(v)) => v.clone(),
                    Some(Nbt::List(v)) => v.iter().filter_map(Nbt::as_i64).collect(),
                    _ => continue,
                };
                if let [lo, hi] = state[..] {
                    sequences.sequences.insert(key.clone(), XoroshiroRandomSource::from_parts(lo, hi));
                }
            }
        }
        sequences
    }

    fn create_sequence(&self, key: &str, world_seed: i64, salt: i32, include_world_seed: bool, include_sequence_id: bool) -> XoroshiroRandomSource {
        let seed = (if include_world_seed { world_seed } else { 0 }) ^ salt as i64;
        create_sequence(seed, include_sequence_id.then_some(key))
    }

    pub fn get(&mut self, key: &str, world_seed: i64) -> &mut XoroshiroRandomSource {
        if !self.sequences.contains_key(key) {
            let sequence = self.create_sequence(key, world_seed, self.salt, self.include_world_seed, self.include_sequence_id);
            self.sequences.insert(key.to_string(), sequence);
        }
        self.dirty = true;
        self.sequences.get_mut(key).unwrap()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.sequences.iter().map(|(k, _)| k)
    }

    pub fn set_seed_defaults(&mut self, salt: i32, include_world_seed: bool, include_sequence_id: bool) {
        self.salt = salt;
        self.include_world_seed = include_world_seed;
        self.include_sequence_id = include_sequence_id;
    }

    pub fn clear(&mut self) -> i32 {
        let count = self.sequences.len() as i32;
        self.sequences.clear();
        count
    }

    pub fn reset(&mut self, id: &str, world_seed: i64) {
        let sequence = self.create_sequence(id, world_seed, self.salt, self.include_world_seed, self.include_sequence_id);
        self.sequences.insert(id.to_string(), sequence);
    }

    pub fn reset_with(&mut self, id: &str, world_seed: i64, salt: i32, include_world_seed: bool, include_sequence_id: bool) {
        let sequence = self.create_sequence(id, world_seed, salt, include_world_seed, include_sequence_id);
        self.sequences.insert(id.to_string(), sequence);
    }

    fn to_nbt(&self) -> Nbt {
        let sequences = self
            .sequences
            .iter()
            .map(|(key, source)| {
                let (lo, hi) = source.state();
                (key.to_string(), Nbt::Compound(vec![("source".into(), Nbt::LongArray(vec![lo, hi]))]))
            })
            .collect();
        Nbt::Compound(vec![
            ("salt".into(), Nbt::Int(self.salt)),
            ("include_world_seed".into(), Nbt::Byte(self.include_world_seed as i8)),
            ("include_sequence_id".into(), Nbt::Byte(self.include_sequence_id as i8)),
            ("sequences".into(), Nbt::Compound(sequences)),
        ])
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let file = Nbt::Compound(vec![("data".into(), self.to_nbt()), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        write_dat(&RandomSequences::file(dir), &file)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::random::RandomSource;

    use super::*;

    #[test]
    fn sequences_match_vanilla() {
        let mut a = create_sequence(42, Some("minecraft:a"));
        let mut b = create_sequence(42 ^ 7, None);
        let a: Vec<i32> = (0..3).map(|_| crisp_core::mth::random_between_inclusive(&mut a, 1, 100)).collect();
        let b: Vec<i32> = (0..3).map(|_| crisp_core::mth::random_between_inclusive(&mut b, -5, 5)).collect();
        assert_eq!((a, b), (vec![49, 82, 30], vec![4, -4, 0]));
    }

    #[test]
    fn sequences_carry_on_after_a_reload() {
        let dir = std::env::temp_dir().join(format!("crisp-random-sequences-{}", std::process::id()));
        let mut sequences = RandomSequences::load(Some(dir.clone()));
        let first = sequences.get("minecraft:a", 42).next_int();
        sequences.save().unwrap();
        let expected = sequences.get("minecraft:a", 42).next_int();
        let mut loaded = RandomSequences::load(Some(dir.clone()));
        assert_eq!(loaded.get("minecraft:a", 42).next_int(), expected);
        assert_ne!(first, expected);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
