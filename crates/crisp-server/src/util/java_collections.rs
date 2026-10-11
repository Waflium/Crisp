use crisp_core::BlockPos;
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

pub trait JavaHashCode {
    fn java_hash_code(&self) -> i32;
}

impl JavaHashCode for BlockPos {
    fn java_hash_code(&self) -> i32 {
        self.y.wrapping_add(self.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(self.x)
    }
}

const NO_NODE: u32 = u32::MAX;

#[derive(Clone)]
struct Node<T> {
    value: T,
    hash: i32,
    next: u32,
}

#[derive(Clone)]
pub struct JavaObjectHashSet<T> {
    table: Vec<u32>,
    nodes: Vec<Node<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Default for JavaObjectHashSet<T> {
    fn default() -> JavaObjectHashSet<T> {
        JavaObjectHashSet { table: vec![NO_NODE; 16], nodes: Vec::new(), free: Vec::new(), len: 0 }
    }
}

impl<T: JavaHashCode + PartialEq + Copy> JavaObjectHashSet<T> {
    fn hash(value: &T) -> i32 {
        let h = value.java_hash_code();
        h ^ ((h as u32) >> 16) as i32
    }

    fn bucket(&self, hash: i32) -> usize {
        hash as u32 as usize & (self.table.len() - 1)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn contains(&self, value: &T) -> bool {
        let hash = Self::hash(value);
        let mut cursor = self.table[self.bucket(hash)];
        while cursor != NO_NODE {
            let node = &self.nodes[cursor as usize];
            if node.hash == hash && node.value == *value {
                return true;
            }
            cursor = node.next;
        }
        false
    }

    pub fn insert(&mut self, value: T) -> bool {
        let hash = Self::hash(&value);
        let index = self.bucket(hash);
        let mut cursor = self.table[index];
        let mut last = NO_NODE;
        let mut bin_count = 0;
        while cursor != NO_NODE {
            let node = &self.nodes[cursor as usize];
            if node.hash == hash && node.value == value {
                return false;
            }
            last = cursor;
            cursor = node.next;
            bin_count += 1;
        }
        let node = Node { value, hash, next: NO_NODE };
        let id = match self.free.pop() {
            Some(id) => {
                self.nodes[id as usize] = node;
                id
            }
            None => {
                self.nodes.push(node);
                (self.nodes.len() - 1) as u32
            }
        };
        if last == NO_NODE {
            self.table[index] = id;
        } else {
            self.nodes[last as usize].next = id;
        }
        if bin_count >= 8 && self.table.len() < 64 {
            self.resize();
        }
        self.len += 1;
        if self.len > self.table.len() * 3 / 4 {
            self.resize();
        }
        true
    }

    fn resize(&mut self) {
        let old_capacity = self.table.len();
        let mut table = vec![NO_NODE; old_capacity * 2];
        for j in 0..old_capacity {
            let (mut lo_tail, mut hi_tail) = (NO_NODE, NO_NODE);
            let mut cursor = self.table[j];
            while cursor != NO_NODE {
                let next = self.nodes[cursor as usize].next;
                self.nodes[cursor as usize].next = NO_NODE;
                let (index, tail) = if self.nodes[cursor as usize].hash as u32 as usize & old_capacity == 0 { (j, &mut lo_tail) } else { (j + old_capacity, &mut hi_tail) };
                if *tail == NO_NODE {
                    table[index] = cursor;
                } else {
                    self.nodes[*tail as usize].next = cursor;
                }
                *tail = cursor;
                cursor = next;
            }
        }
        self.table = table;
    }

    pub fn remove(&mut self, value: &T) -> bool {
        let hash = Self::hash(value);
        let index = self.bucket(hash);
        let mut cursor = self.table[index];
        let mut previous = NO_NODE;
        while cursor != NO_NODE {
            let node = &self.nodes[cursor as usize];
            if node.hash == hash && node.value == *value {
                let next = node.next;
                if previous == NO_NODE {
                    self.table[index] = next;
                } else {
                    self.nodes[previous as usize].next = next;
                }
                self.free.push(cursor);
                self.len -= 1;
                return true;
            }
            previous = cursor;
            cursor = node.next;
        }
        false
    }

    pub fn first(&self) -> Option<T> {
        self.table.iter().find(|&&head| head != NO_NODE).map(|&head| self.nodes[head as usize].value)
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.table.iter().flat_map(move |&head| {
            std::iter::successors((head != NO_NODE).then_some(head), move |&cursor| {
                let next = self.nodes[cursor as usize].next;
                (next != NO_NODE).then_some(next)
            })
            .map(move |cursor| self.nodes[cursor as usize].value)
        })
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
