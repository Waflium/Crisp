use std::sync::atomic::{AtomicU64, Ordering};

pub struct Tag {
    pub name: &'static str,
    bits: &'static [AtomicU64],
}

impl Tag {
    pub const fn new(name: &'static str, bits: &'static [AtomicU64]) -> Tag {
        Tag { name, bits }
    }

    pub fn contains(&self, id: usize) -> bool {
        self.bits.get(id / 64).is_some_and(|w| w.load(Ordering::Relaxed) & (1 << (id % 64)) != 0)
    }

    pub fn bind(&self, values: &[i32]) {
        let mut bits = vec![0u64; self.bits.len()];
        for &value in values {
            if let Some(word) = bits.get_mut(value as usize / 64) {
                *word |= 1 << (value as usize % 64);
            }
        }
        for (word, bits) in self.bits.iter().zip(bits) {
            word.store(bits, Ordering::Relaxed);
        }
    }
}
