use crisp_core::random::java_string_hash;
use indexmap::IndexSet;

#[derive(Clone, Debug)]
pub struct JavaHashSet {
    entries: IndexSet<String>,
    capacity: usize,
}

impl Default for JavaHashSet {
    fn default() -> JavaHashSet {
        JavaHashSet { entries: IndexSet::new(), capacity: 16 }
    }
}

impl JavaHashSet {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn insert(&mut self, value: String) -> bool {
        if !self.entries.insert(value) {
            return false;
        }
        if self.entries.len() > self.capacity * 3 / 4 {
            self.capacity *= 2;
        }
        true
    }

    pub fn remove(&mut self, value: &str) -> bool {
        self.entries.shift_remove(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let bucket = |s: &str| {
            let h = java_string_hash(s);
            (h ^ ((h as u32) >> 16) as i32) as u32 as usize & (self.capacity - 1)
        };
        let mut order: Vec<&str> = self.entries.iter().map(String::as_str).collect();
        order.sort_by_key(|s| bucket(s));
        order.into_iter()
    }
}

pub fn identifier_hash(id: &str) -> i32 {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    java_string_hash(namespace).wrapping_mul(31).wrapping_add(java_string_hash(path))
}

#[derive(Clone, Debug)]
pub struct JavaHashMap<V> {
    entries: indexmap::IndexMap<String, V>,
    capacity: usize,
    hash: fn(&str) -> i32,
}

impl<V> JavaHashMap<V> {
    pub fn new(hash: fn(&str) -> i32) -> JavaHashMap<V> {
        JavaHashMap { entries: indexmap::IndexMap::new(), capacity: 16, hash }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries.get_mut(key)
    }

    pub fn insert(&mut self, key: String, value: V) -> Option<V> {
        let old = self.entries.insert(key, value);
        if old.is_none() && self.entries.len() > self.capacity * 3 / 4 {
            self.capacity *= 2;
        }
        old
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        self.entries.shift_remove(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        let bucket = |s: &str| {
            let h = (self.hash)(s);
            (h ^ ((h as u32) >> 16) as i32) as u32 as usize & (self.capacity - 1)
        };
        let mut order: Vec<(&str, &V)> = self.entries.iter().map(|(k, v)| (k.as_str(), v)).collect();
        order.sort_by_key(|(k, _)| bucket(k));
        order.into_iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.entries.values_mut()
    }
}
