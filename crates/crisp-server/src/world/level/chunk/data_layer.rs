use std::sync::atomic::{AtomicU64, Ordering};

pub const SIZE: usize = 2048;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub struct DataLayer {
    data: Option<Box<[u8; SIZE]>>,
    default_value: u8,
    id: u64,
}

impl Default for DataLayer {
    fn default() -> Self {
        DataLayer::filled(0)
    }
}

fn index(x: i32, y: i32, z: i32) -> usize {
    (y << 8 | z << 4 | x) as usize
}

fn pack_filled(value: u8) -> u8 {
    value & 15 | value << 4
}

impl DataLayer {
    pub fn new() -> DataLayer {
        DataLayer::filled(0)
    }

    pub fn filled(default_value: u8) -> DataLayer {
        DataLayer { data: None, default_value, id: next_id() }
    }

    pub fn from_bytes(data: Box<[u8; SIZE]>) -> DataLayer {
        DataLayer { data: Some(data), default_value: 0, id: next_id() }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.get_index(index(x, y, z))
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, value: u8) {
        let i = index(x, y, z);
        let shift = 4 * (i & 1);
        let data = self.data_mut();
        data[i >> 1] = data[i >> 1] & !(15 << shift) | (value & 15) << shift;
    }

    fn get_index(&self, i: usize) -> u8 {
        match &self.data {
            None => self.default_value,
            Some(data) => data[i >> 1] >> (4 * (i & 1)) & 15,
        }
    }

    pub fn fill(&mut self, value: u8) {
        self.default_value = value;
        self.data = None;
    }

    fn data_mut(&mut self) -> &mut [u8; SIZE] {
        let default_value = self.default_value;
        self.data.get_or_insert_with(|| Box::new([pack_filled(default_value); SIZE]))
    }

    pub fn to_bytes(&self) -> Box<[u8; SIZE]> {
        match &self.data {
            Some(data) => data.clone(),
            None => Box::new([pack_filled(self.default_value); SIZE]),
        }
    }

    pub fn copy(&self) -> DataLayer {
        DataLayer { data: self.data.clone(), default_value: self.default_value, id: next_id() }
    }

    pub fn repeat_first_layer(&self) -> DataLayer {
        let Some(data) = &self.data else { return self.copy() };
        let mut output = Box::new([0u8; SIZE]);
        for i in 0..16 {
            output[i * 128..(i + 1) * 128].copy_from_slice(&data[..128]);
        }
        DataLayer::from_bytes(output)
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_none() && self.default_value == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_materialize_the_default() {
        let mut layer = DataLayer::filled(15);
        assert_eq!(layer.to_bytes()[0], 0xFF);
        layer.set(1, 0, 0, 3);
        assert_eq!(layer.get(0, 0, 0), 15);
        assert_eq!(layer.get(1, 0, 0), 3);
        assert_eq!(layer.to_bytes()[0], 0x3F);
        let mut zero = DataLayer::new();
        assert!(zero.is_empty());
        zero.set(0, 0, 0, 0);
        assert!(!zero.is_empty(), "a written layer is never empty, even if all zero");
    }

    #[test]
    fn copies_are_new_objects() {
        let layer = DataLayer::new();
        assert_ne!(layer.copy().id(), layer.id());
        assert_eq!(layer.clone().id(), layer.id());
    }
}
