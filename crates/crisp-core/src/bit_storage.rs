#[derive(Clone, Debug, PartialEq)]
pub enum BitStorage {
    Zero { size: usize },
    Simple(SimpleBitStorage),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimpleBitStorage {
    data: Box<[i64]>,
    bits: u32,
    mask: u64,
    size: usize,
    values_per_long: usize,
}

impl SimpleBitStorage {
    pub fn new(bits: u32, size: usize) -> SimpleBitStorage {
        assert!((1..=32).contains(&bits), "bits out of range: {bits}");
        let values_per_long = 64 / bits as usize;
        SimpleBitStorage { data: vec![0; size.div_ceil(values_per_long)].into(), bits, mask: (1 << bits) - 1, size, values_per_long }
    }

    pub fn with_data(bits: u32, size: usize, data: Vec<i64>) -> Option<SimpleBitStorage> {
        let mut storage = SimpleBitStorage::new(bits, size);
        if data.len() != storage.data.len() {
            return None;
        }
        storage.data = data.into();
        Some(storage)
    }

    pub fn from_values(bits: u32, values: &[u32]) -> SimpleBitStorage {
        let mut storage = SimpleBitStorage::new(bits, values.len());
        for (i, &v) in values.iter().enumerate() {
            storage.set(i, v);
        }
        storage
    }

    fn locate(&self, index: usize) -> (usize, u32) {
        assert!(index < self.size, "index {index} out of range");
        let cell = index / self.values_per_long;
        (cell, ((index - cell * self.values_per_long) as u32) * self.bits)
    }

    pub fn get(&self, index: usize) -> u32 {
        let (cell, shift) = self.locate(index);
        ((self.data[cell] as u64 >> shift) & self.mask) as u32
    }

    pub fn get_and_set(&mut self, index: usize, value: u32) -> u32 {
        let old = self.get(index);
        self.set(index, value);
        old
    }

    pub fn set(&mut self, index: usize, value: u32) {
        assert!(value as u64 <= self.mask, "value {value} does not fit in {} bits", self.bits);
        let (cell, shift) = self.locate(index);
        let v = self.data[cell] as u64;
        self.data[cell] = (v & !(self.mask << shift) | (value as u64 & self.mask) << shift) as i64;
    }

    pub fn fill(&mut self, value: u32) {
        let mut cell = value as u64 & self.mask;
        let mut offset = self.bits;
        while offset < 64 {
            cell |= cell << offset;
            offset <<= 1;
        }
        self.data.fill(cell as i64);
    }

    pub fn raw(&self) -> &[i64] {
        &self.data
    }

    pub fn raw_mut(&mut self) -> &mut [i64] {
        &mut self.data
    }

    pub fn bits(&self) -> u32 {
        self.bits
    }
}

impl BitStorage {
    pub fn new(bits: u32, size: usize) -> BitStorage {
        if bits == 0 { BitStorage::Zero { size } } else { BitStorage::Simple(SimpleBitStorage::new(bits, size)) }
    }

    pub fn size(&self) -> usize {
        match self {
            BitStorage::Zero { size } => *size,
            BitStorage::Simple(s) => s.size,
        }
    }

    pub fn bits(&self) -> u32 {
        match self {
            BitStorage::Zero { .. } => 0,
            BitStorage::Simple(s) => s.bits,
        }
    }

    pub fn get(&self, index: usize) -> u32 {
        match self {
            BitStorage::Zero { size } => {
                assert!(index < *size);
                0
            }
            BitStorage::Simple(s) => s.get(index),
        }
    }

    pub fn set(&mut self, index: usize, value: u32) {
        match self {
            BitStorage::Zero { size } => assert!(index < *size && value == 0),
            BitStorage::Simple(s) => s.set(index, value),
        }
    }

    pub fn get_and_set(&mut self, index: usize, value: u32) -> u32 {
        match self {
            BitStorage::Zero { size } => {
                assert!(index < *size && value == 0);
                0
            }
            BitStorage::Simple(s) => s.get_and_set(index, value),
        }
    }

    pub fn fill(&mut self, value: u32) {
        match self {
            BitStorage::Zero { .. } => assert_eq!(value, 0),
            BitStorage::Simple(s) => s.fill(value),
        }
    }

    pub fn raw(&self) -> &[i64] {
        match self {
            BitStorage::Zero { .. } => &[],
            BitStorage::Simple(s) => s.raw(),
        }
    }

    pub fn unpack(&self) -> Vec<u32> {
        (0..self.size()).map(|i| self.get(i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_like_the_chunk_format() {
        let values = [1, 2, 2, 3, 4, 4, 5, 6, 6, 4, 8, 0, 7, 4, 3, 13, 15, 16, 9, 14, 10, 12, 0, 2];
        let s = SimpleBitStorage::from_values(5, &values);
        assert_eq!(s.raw(), &[0x0020863148418841, 0x01018A7260F68C87]);
        assert_eq!((0..24).map(|i| s.get(i)).collect::<Vec<_>>(), values);
    }
}
