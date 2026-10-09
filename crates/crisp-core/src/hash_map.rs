use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default, Clone, Copy)]
pub struct FastHasher(u64);

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FastHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, n: u8) {
        self.add(n as u64);
    }

    fn write_u16(&mut self, n: u16) {
        self.add(n as u64);
    }

    fn write_u32(&mut self, n: u32) {
        self.add(n as u64);
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    fn write_i32(&mut self, n: i32) {
        self.add(n as u32 as u64);
    }

    fn write_i64(&mut self, n: i64) {
        self.add(n as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;
pub type FastSet<K> = HashSet<K, BuildHasherDefault<FastHasher>>;

fn spread(hash: i32) -> u32 {
    let h = hash as u32;
    h ^ (h >> 16)
}

pub fn java_hash_map_order(hashes: &[i32]) -> Vec<usize> {
    let mut capacity = 16usize;
    while hashes.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let mut order: Vec<usize> = (0..hashes.len()).collect();
    order.sort_by_key(|&i| spread(hashes[i]) as usize & (capacity - 1));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_then_insertion_order() {
        assert_eq!(java_hash_map_order(&[17, 0, 1]), [1, 0, 2]);
        let mut hashes = vec![17, 1];
        hashes.extend(100..111);
        let mut expected: Vec<usize> = (1..13).collect();
        expected.push(0);
        assert_eq!(java_hash_map_order(&hashes), expected);
    }
}
