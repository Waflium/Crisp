use std::collections::HashMap;

use crate::random::java_string_hash;

pub fn mix(x: i64) -> i64 {
    let h = x.wrapping_mul(-7046029254386353131);
    let h = h ^ ((h as u64) >> 32) as i64;
    h ^ ((h as u64) >> 16) as i64
}

fn next_power_of_two(x: i64) -> i64 {
    1i64.wrapping_shl(64 - (x.wrapping_sub(1)).leading_zeros())
}

pub fn max_fill(n: usize, f: f32) -> usize {
    ((n as f64 * f as f64).ceil() as usize).min(n - 1)
}

pub fn array_size(expected: usize, f: f32) -> usize {
    2.max(next_power_of_two((expected as f64 / f as f64).ceil() as i64)) as usize
}

pub trait OpenHashKey: Copy + Eq {
    const NULL: Self;
    fn mix(self) -> usize;
}

impl OpenHashKey for i64 {
    const NULL: i64 = 0;

    fn mix(self) -> usize {
        mix(self) as usize
    }
}

impl OpenHashKey for i32 {
    const NULL: i32 = 0;

    fn mix(self) -> usize {
        mix_int(self) as u32 as usize
    }
}

impl OpenHashKey for i16 {
    const NULL: i16 = 0;

    fn mix(self) -> usize {
        mix_int(self as i32) as u32 as usize
    }
}

impl OpenHashKey for crate::BlockPos {
    const NULL: crate::BlockPos = crate::BlockPos::new(i32::MIN, i32::MIN, i32::MIN);

    fn mix(self) -> usize {
        mix_int(self.y.wrapping_add(self.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(self.x)) as u32 as usize
    }
}

pub type Long2ObjectOpenHashMap<V> = OpenHashMap<i64, V>;
pub type Int2ObjectOpenHashMap<V> = OpenHashMap<i32, V>;

#[derive(Clone)]
pub struct OpenHashMap<K, V> {
    key: Vec<K>,
    value: Vec<Option<V>>,
    mask: usize,
    contains_null_key: bool,
    n: usize,
    max_fill: usize,
    min_n: usize,
    size: usize,
    f: f32,
}

impl<K: OpenHashKey, V> Default for OpenHashMap<K, V> {
    fn default() -> Self {
        OpenHashMap::new()
    }
}

impl<K: OpenHashKey, V> OpenHashMap<K, V> {
    pub fn new() -> Self {
        Self::with_expected(16, 0.75)
    }

    pub fn with_expected(expected: usize, f: f32) -> Self {
        let n = array_size(expected, f);
        OpenHashMap {
            key: vec![K::NULL; n + 1],
            value: (0..=n).map(|_| None).collect(),
            mask: n - 1,
            contains_null_key: false,
            n,
            max_fill: max_fill(n, f),
            min_n: n,
            size: 0,
            f,
        }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn slot(&self, k: K) -> usize {
        k.mix() & self.mask
    }

    fn find(&self, k: K) -> Result<usize, usize> {
        if k == K::NULL {
            return if self.contains_null_key { Ok(self.n) } else { Err(self.n) };
        }
        let mut pos = self.slot(k);
        loop {
            let curr = self.key[pos];
            if curr == K::NULL {
                return Err(pos);
            }
            if curr == k {
                return Ok(pos);
            }
            pos = (pos + 1) & self.mask;
        }
    }

    fn insert_at(&mut self, pos: usize, k: K, v: V) {
        if pos == self.n {
            self.contains_null_key = true;
        }
        self.key[pos] = k;
        self.value[pos] = Some(v);
        let old = self.size;
        self.size += 1;
        if old >= self.max_fill {
            self.rehash(array_size(self.size + 1, self.f));
        }
    }

    pub fn get(&self, k: K) -> Option<&V> {
        self.find(k).ok().and_then(|pos| self.value[pos].as_ref())
    }

    pub fn get_mut(&mut self, k: K) -> Option<&mut V> {
        self.find(k).ok().and_then(|pos| self.value[pos].as_mut())
    }

    pub fn contains_key(&self, k: K) -> bool {
        self.find(k).is_ok()
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        match self.find(k) {
            Ok(pos) => self.value[pos].replace(v),
            Err(pos) => {
                self.insert_at(pos, k, v);
                None
            }
        }
    }

    pub fn get_or_insert_with(&mut self, k: K, f: impl FnOnce() -> V) -> &mut V {
        let pos = match self.find(k) {
            Ok(pos) => pos,
            Err(pos) => {
                self.insert_at(pos, k, f());
                self.find(k).ok().unwrap()
            }
        };
        self.value[pos].as_mut().unwrap()
    }

    pub fn remove(&mut self, k: K) -> Option<V> {
        if k == K::NULL {
            return if self.contains_null_key { self.remove_null_entry() } else { None };
        }
        let pos = self.find(k).ok()?;
        self.remove_entry(pos)
    }

    fn shrink_if_sparse(&mut self) {
        if self.n > self.min_n && self.size < self.max_fill / 4 && self.n > 16 {
            self.rehash(self.n / 2);
        }
    }

    fn remove_entry(&mut self, pos: usize) -> Option<V> {
        let old = self.value[pos].take();
        self.size -= 1;
        self.shift_keys(pos, None);
        self.shrink_if_sparse();
        old
    }

    fn remove_null_entry(&mut self) -> Option<V> {
        self.contains_null_key = false;
        let old = self.value[self.n].take();
        self.size -= 1;
        self.shrink_if_sparse();
        old
    }

    fn shift_keys(&mut self, mut pos: usize, mut wrapped: Option<&mut Vec<K>>) {
        loop {
            let last = pos;
            pos = (pos + 1) & self.mask;
            loop {
                let curr = self.key[pos];
                if curr == K::NULL {
                    self.key[last] = K::NULL;
                    self.value[last] = None;
                    return;
                }
                let slot = self.slot(curr);
                let moves = if last <= pos { last >= slot || slot > pos } else { last >= slot && slot > pos };
                if moves {
                    if pos < last
                        && let Some(wrapped) = wrapped.as_deref_mut()
                    {
                        wrapped.push(curr);
                    }
                    self.key[last] = curr;
                    self.value[last] = self.value[pos].take();
                    break;
                }
                pos = (pos + 1) & self.mask;
            }
        }
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut new_key = vec![K::NULL; new_n + 1];
        let mut new_value: Vec<Option<V>> = (0..=new_n).map(|_| None).collect();
        let mut i = self.n;
        let real_size = self.size - usize::from(self.contains_null_key);
        for _ in 0..real_size {
            loop {
                i -= 1;
                if self.key[i] != K::NULL {
                    break;
                }
            }
            let mut pos = self.key[i].mix() & mask;
            while new_key[pos] != K::NULL {
                pos = (pos + 1) & mask;
            }
            new_key[pos] = self.key[i];
            new_value[pos] = self.value[i].take();
        }
        new_value[new_n] = self.value[self.n].take();
        self.n = new_n;
        self.mask = mask;
        self.max_fill = max_fill(new_n, self.f);
        self.key = new_key;
        self.value = new_value;
    }

    pub fn clear(&mut self) {
        if self.size != 0 {
            self.size = 0;
            self.contains_null_key = false;
            self.key.iter_mut().for_each(|k| *k = K::NULL);
            self.value.iter_mut().for_each(|v| *v = None);
        }
    }

    fn positions(&self) -> impl Iterator<Item = usize> + '_ {
        let null = self.contains_null_key.then_some(self.n);
        null.into_iter().chain((0..self.n).rev().filter(|&pos| self.key[pos] != K::NULL))
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + '_ {
        self.positions().map(|pos| (self.key[pos], self.value[pos].as_ref().unwrap()))
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.positions().map(|pos| self.key[pos])
    }

    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.positions().map(|pos| self.value[pos].as_ref().unwrap())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> + '_ {
        self.value.iter_mut().filter_map(Option::as_mut)
    }

    pub fn stream_values(&self) -> impl Iterator<Item = &V> + '_ {
        let null = self.contains_null_key.then_some(self.n);
        null.into_iter().chain((0..self.n).filter(|&pos| self.key[pos] != K::NULL)).map(|pos| self.value[pos].as_ref().unwrap())
    }

    pub fn retain(&mut self, mut keep: impl FnMut(K, &mut V) -> bool) {
        let mut pos = self.n as isize;
        let mut remaining = self.size;
        let mut must_return_null = self.contains_null_key;
        let mut wrapped: Vec<K> = Vec::new();
        while remaining != 0 {
            remaining -= 1;
            let (index, last) = if must_return_null {
                must_return_null = false;
                (self.n, Some(self.n))
            } else {
                let mut found = None;
                loop {
                    pos -= 1;
                    if pos < 0 {
                        break;
                    }
                    if self.key[pos as usize] != K::NULL {
                        found = Some(pos as usize);
                        break;
                    }
                }
                match found {
                    Some(p) => (p, Some(p)),
                    None => {
                        let k = wrapped[(-pos - 1) as usize];
                        let mut p = self.slot(k);
                        while self.key[p] != k {
                            p = (p + 1) & self.mask;
                        }
                        (p, None)
                    }
                }
            };
            let k = self.key[index];
            if keep(k, self.value[index].as_mut().unwrap()) {
                continue;
            }
            match last {
                Some(last) if last == self.n => {
                    self.contains_null_key = false;
                    self.value[self.n] = None;
                }
                Some(last) => self.shift_keys(last, Some(&mut wrapped)),
                None => {
                    self.remove(wrapped[(-pos - 1) as usize]);
                    continue;
                }
            }
            self.size -= 1;
        }
    }
}

#[derive(Clone, Default)]
pub struct LongOpenHashSet(Long2ObjectOpenHashMap<()>);

impl LongOpenHashSet {
    pub fn new() -> Self {
        LongOpenHashSet(Long2ObjectOpenHashMap::new())
    }

    pub fn with_expected(expected: usize, f: f32) -> Self {
        LongOpenHashSet(Long2ObjectOpenHashMap::with_expected(expected, f))
    }

    pub fn add(&mut self, k: i64) -> bool {
        match self.0.find(k) {
            Ok(_) => false,
            Err(pos) => {
                self.0.insert_at(pos, k, ());
                true
            }
        }
    }

    pub fn remove(&mut self, k: i64) -> bool {
        self.0.remove(k).is_some()
    }

    pub fn contains(&self, k: i64) -> bool {
        self.0.contains_key(k)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = i64> + '_ {
        self.0.keys()
    }

    pub fn retain(&mut self, mut keep: impl FnMut(i64) -> bool) {
        self.0.retain(|k, _| keep(k));
    }
}

#[derive(Clone, Default)]
pub struct ShortOpenHashSet(OpenHashMap<i16, ()>);

impl ShortOpenHashSet {
    pub fn add(&mut self, k: i16) -> bool {
        match self.0.find(k) {
            Ok(_) => false,
            Err(pos) => {
                self.0.insert_at(pos, k, ());
                true
            }
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = i16> + '_ {
        self.0.keys()
    }
}

#[derive(Clone)]
pub struct Long2ByteOpenHashMap {
    map: Long2ObjectOpenHashMap<i8>,
    default_return_value: i8,
}

impl Long2ByteOpenHashMap {
    pub fn new(default_return_value: i8) -> Self {
        Long2ByteOpenHashMap { map: Long2ObjectOpenHashMap::new(), default_return_value }
    }

    pub fn get(&self, k: i64) -> i8 {
        self.map.get(k).copied().unwrap_or(self.default_return_value)
    }

    pub fn put(&mut self, k: i64, v: i8) -> i8 {
        self.map.insert(k, v).unwrap_or(self.default_return_value)
    }

    pub fn remove(&mut self, k: i64) -> i8 {
        self.map.remove(k).unwrap_or(self.default_return_value)
    }

    pub fn contains_key(&self, k: i64) -> bool {
        self.map.contains_key(k)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (i64, i8)> + '_ {
        self.map.iter().map(|(k, &v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = i64> + '_ {
        self.map.keys()
    }
}

#[derive(Clone, Default)]
pub struct LongLinkedOpenHashSet {
    links: HashMap<i64, (Option<i64>, Option<i64>)>,
    first: Option<i64>,
    last: Option<i64>,
}

impl LongLinkedOpenHashSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, k: i64) -> bool {
        if self.links.contains_key(&k) {
            return false;
        }
        self.links.insert(k, (self.last, None));
        match self.last {
            Some(last) => self.links.get_mut(&last).unwrap().1 = Some(k),
            None => self.first = Some(k),
        }
        self.last = Some(k);
        true
    }

    pub fn remove(&mut self, k: i64) -> bool {
        let Some((prev, next)) = self.links.remove(&k) else { return false };
        match prev {
            Some(p) => self.links.get_mut(&p).unwrap().1 = next,
            None => self.first = next,
        }
        match next {
            Some(n) => self.links.get_mut(&n).unwrap().0 = prev,
            None => self.last = prev,
        }
        true
    }

    pub fn remove_first(&mut self) -> Option<i64> {
        let first = self.first?;
        self.remove(first);
        Some(first)
    }

    pub fn first(&self) -> Option<i64> {
        self.first
    }

    pub fn contains(&self, k: i64) -> bool {
        self.links.contains_key(&k)
    }

    pub fn len(&self) -> usize {
        self.links.len()
    }

    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = i64> + '_ {
        let mut next = self.first;
        std::iter::from_fn(move || {
            let k = next?;
            next = self.links[&k].1;
            Some(k)
        })
    }
}

pub fn mix_int(x: i32) -> i32 {
    let h = x.wrapping_mul(0x9E3779B9u32 as i32);
    h ^ ((h as u32) >> 16) as i32
}

#[derive(Clone, Debug)]
pub struct Object2ObjectOpenHashMap<V> {
    slots: Vec<Option<(i32, String, V)>>,
    mask: usize,
    min_n: usize,
    max_fill: usize,
    size: usize,
    f: f32,
    hash: fn(&str) -> i32,
}

impl<V> Object2ObjectOpenHashMap<V> {
    pub fn new(expected: usize, f: f32) -> Object2ObjectOpenHashMap<V> {
        Object2ObjectOpenHashMap::with_hash(expected, f, java_string_hash)
    }

    pub fn with_hash(expected: usize, f: f32, hash: fn(&str) -> i32) -> Object2ObjectOpenHashMap<V> {
        let n = array_size(expected, f);
        Object2ObjectOpenHashMap { slots: (0..n).map(|_| None).collect(), mask: n - 1, min_n: n, max_fill: max_fill(n, f), size: 0, f, hash }
    }

    pub fn clear(&mut self) {
        self.slots.iter_mut().for_each(|s| *s = None);
        self.size = 0;
    }

    pub fn len(&self) -> usize {
        self.size
    }

    fn find(&self, key: &str, hash: i32) -> Result<usize, usize> {
        let mut pos = mix_int(hash) as usize & self.mask;
        loop {
            match &self.slots[pos] {
                None => return Err(pos),
                Some((h, k, _)) if *h == hash && k == key => return Ok(pos),
                Some(_) => pos = (pos + 1) & self.mask,
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        let pos = self.find(key, (self.hash)(key)).ok()?;
        self.slots[pos].as_ref().map(|s| &s.2)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        let pos = self.find(key, (self.hash)(key)).ok()?;
        self.slots[pos].as_mut().map(|s| &mut s.2)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.find(key, (self.hash)(key)).is_ok()
    }

    pub fn insert(&mut self, key: String, value: V) -> Option<V> {
        let hash = (self.hash)(&key);
        match self.find(&key, hash) {
            Ok(pos) => Some(std::mem::replace(&mut self.slots[pos].as_mut().unwrap().2, value)),
            Err(pos) => {
                self.slots[pos] = Some((hash, key, value));
                let size = self.size;
                self.size += 1;
                if size >= self.max_fill {
                    self.rehash(array_size(self.size + 1, self.f));
                }
                None
            }
        }
    }

    pub fn get_or_insert_with(&mut self, key: &str, make: impl FnOnce() -> V) -> &mut V {
        let hash = (self.hash)(key);
        let pos = match self.find(key, hash) {
            Ok(pos) => pos,
            Err(_) => {
                self.insert(key.to_string(), make());
                self.find(key, hash).unwrap()
            }
        };
        &mut self.slots[pos].as_mut().unwrap().2
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        let pos = self.find(key, (self.hash)(key)).ok()?;
        let (_, _, value) = self.slots[pos].take().unwrap();
        self.size -= 1;
        self.shift_keys(pos);
        let n = self.mask + 1;
        if n > self.min_n && self.size < self.max_fill / 4 && n > 16 {
            self.rehash(n / 2);
        }
        Some(value)
    }

    fn shift_keys(&mut self, mut pos: usize) {
        loop {
            let last = pos;
            pos = (last + 1) & self.mask;
            loop {
                let Some((hash, _, _)) = &self.slots[pos] else {
                    self.slots[last] = None;
                    return;
                };
                let slot = mix_int(*hash) as usize & self.mask;
                let stays = if last <= pos { last >= slot || slot > pos } else { last >= slot && slot > pos };
                if stays {
                    break;
                }
                pos = (pos + 1) & self.mask;
            }
            self.slots[last] = self.slots[pos].take();
        }
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut slots: Vec<Option<(i32, String, V)>> = (0..new_n).map(|_| None).collect();
        for entry in std::mem::take(&mut self.slots).into_iter().rev().flatten() {
            let mut pos = mix_int(entry.0) as usize & mask;
            while slots[pos].is_some() {
                pos = (pos + 1) & mask;
            }
            slots[pos] = Some(entry);
        }
        self.slots = slots;
        self.mask = mask;
        self.max_fill = max_fill(new_n, self.f);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.slots.iter().rev().flatten().map(|(_, k, v)| (k.as_str(), v))
    }

    pub fn stream(&self) -> impl Iterator<Item = (&str, &V)> {
        self.slots.iter().flatten().map(|(_, k, v)| (k.as_str(), v))
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.slots.iter_mut().rev().flatten().map(|(_, _, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{LegacyRandomSource, RandomSource};

    fn order_hash(keys: impl Iterator<Item = i64>) -> i64 {
        keys.fold(0i64, |h, k| h.wrapping_mul(31).wrapping_add(k))
    }

    fn key(random: &mut LegacyRandomSource, radius: i32) -> i64 {
        let x = random.next_int_bounded(radius * 2 + 1) - radius;
        let z = random.next_int_bounded(radius * 2 + 1) - radius;
        (x as i64 & 0xFFFF_FFFF) | ((z as i64 & 0xFFFF_FFFF) << 32)
    }

    #[test]
    fn mix_matches_hash_common() {
        assert_eq!(mix(0), 0);
        assert_eq!(mix(1), -7045908492363438895);
        assert_eq!(mix(-5), -1663385673677052010);
        assert_eq!(array_size(16, 0.75), 32);
        assert_eq!(array_size(4, 0.75), 8);
        assert_eq!(array_size(0, 0.75), 2);
        assert_eq!(max_fill(32, 0.75), 24);
    }

    fn run(seed: i64, rounds: usize, radius: i32) -> Vec<(i64, i64, i64)> {
        let mut random = LegacyRandomSource::new(seed);
        let mut set = LongOpenHashSet::new();
        let mut bytes = Long2ByteOpenHashMap::new(33);
        let mut objects: Long2ObjectOpenHashMap<Vec<i64>> = Long2ObjectOpenHashMap::with_expected(4, 0.75);
        let mut out = Vec::new();
        for _ in 0..rounds {
            for _ in 0..64 {
                let k = key(&mut random, radius);
                let op = random.next_int_bounded(5);
                if op < 3 {
                    set.add(k);
                    bytes.put(k, op as i8);
                    objects.get_or_insert_with(k, Vec::new).push(k);
                } else {
                    set.remove(k);
                    bytes.remove(k);
                    objects.remove(k);
                }
            }
            out.push((order_hash(set.iter()), order_hash(bytes.iter().map(|(k, v)| k ^ v as i64)), order_hash(objects.keys())));
        }
        let mut flip = false;
        set.retain(|_| {
            flip = !flip;
            flip
        });
        let mut visited = Vec::new();
        objects.retain(|k, _| {
            visited.push(k);
            false
        });
        out.push((order_hash(set.iter()), set.len() as i64, order_hash(visited.into_iter())));
        out
    }

    #[test]
    fn iteration_orders_match_fastutil() {
        let expected: &[(i64, i64, i64)] = &ORACLE;
        let mut got = run(1, 8, 3);
        got.extend(run(2, 40, 12));
        got.extend(run(3, 20, 40));
        assert_eq!(got, expected);
    }

    #[test]
    fn linked_set_keeps_insertion_order() {
        let mut set = LongLinkedOpenHashSet::new();
        for k in [5, 3, 9, 1] {
            set.add(k);
        }
        set.add(3);
        set.remove(9);
        set.add(9);
        assert_eq!(set.iter().collect::<Vec<_>>(), [5, 3, 1, 9]);
        assert_eq!(set.remove_first(), Some(5));
        assert_eq!(set.iter().collect::<Vec<_>>(), [3, 1, 9]);
    }

    #[rustfmt::skip]
    const ORACLE: [(i64, i64, i64); 71] = [
        (1310132889587382006, -1347381831983480207, 1310132760738366966),
        (5445439127259715533, 678608919331441905, 5445439127256021573),
        (477501231797503976, 2001243096539134406, 477501231797503976),
        (-2764085264759089331, 38268501312008978, -2764085264759089331),
        (4305621336826112139, 7976554521848930206, 4305621336826112139),
        (-7245838503508457218, -5421395288357437956, -7245838503508457218),
        (-5061172190244245618, 3872032751561492458, -5061172190244245618),
        (-3597032814611496075, -1954840988990568978, -3597032814611496075),
        (-8111356052697598243, 16, -3597032814611496075),
        (1313359837822268779, 5177712445817952046, 2128623726762804331),
        (-1447146241559935045, -8968360807251685231, 9049274538063500475),
        (-2551522662362323456, 4511773404875055079, -6309105252395767160),
        (-5404389453490349352, 4159909441112215085, -5404389453490349352),
        (5671210160918194051, 3775930193527524961, 5671210160918194051),
        (-93935881084975110, 8415884572534917955, -93935881084975110),
        (6362605358538714782, 8028406336595633924, 6362605358538714782),
        (-1497591995349615717, -6030303721712605831, -1497591995349615717),
        (4656075686013530341, 949904176958058735, 4656075686013530341),
        (-3505943311020241647, 13517028855940916, -3505943311020241647),
        (-4107804122769681376, -4196732857962496656, -4107804122769681376),
        (7353783772130044061, 2745518169107606848, 7353783772130044061),
        (-1393176775150599705, -4584780385404240418, -1393176775150599705),
        (4603835050505864275, -8077330605952331924, 4603835050505864275),
        (-556049442636884508, -691187527035939820, -556049442636884508),
        (5359618686670887250, -7673842300289709332, 5359618686670887250),
        (7224080961908624276, 7148938639459603941, 7224080961908624276),
        (-5569013703938693019, -4217777948086988989, -5569013703938693019),
        (-6488686817303143489, 6109447608172051299, -6488686817303143489),
        (-203180623705655503, -8651589699646064374, -203180623705655503),
        (-4981027746180346184, -2989222460174231539, -4981027746180346184),
        (-7745333137950720570, 2505429677778365852, -7745333137950720570),
        (967240120408048251, 4055078321328304492, 967240120408048251),
        (1056129547923266102, 2126497753916957190, 1056129547923266102),
        (-7776771053369499558, -5650412008408688039, -7776771053369499558),
        (2375320972217406715, -3900498595597410624, 2375320972217406715),
        (6710876651058079476, 8786913471475451185, 6710876651058079476),
        (3368070690153396161, 2413390896122147711, 3368070690153396161),
        (2376586267743743182, -2871019778737468034, 2376586267743743182),
        (1997118446754765393, 5386079771295483931, 1997118446754765393),
        (3927741397128400708, -2928057145763961897, 3927741397128400708),
        (-2669983510320779181, 1768160041486640624, -2669983510320779181),
        (-2801097765255872368, -8023307005627009258, -2801097765255872368),
        (6184145073579925573, 5056782829249293077, 6184145073579925573),
        (2309079076614681793, 2219790708056003558, 2309079076614681793),
        (-1105117717571409603, -1235983318718185, -1105117717571409603),
        (2746540159734931824, 4865739218457614972, 2746540159734931824),
        (2693943154195143986, -5787133253020677276, 2693943154195143986),
        (-3787159620177424251, -3098101995316747825, -3787159620177424251),
        (6973307533237288570, 7054341293126101158, 6973307533237288570),
        (7056432767017727209, 181, 6973307533237288570),
        (-1320922551480868143, -2657228760944281506, -2164025087992319279),
        (3657566560920775140, -1096311593172837542, 6426539945495936420),
        (-8545438145942980953, 7421907953519628018, 3665193835039980305),
        (8091614830765741845, -4230242215411920383, -1614773237529701909),
        (-2520364922334079135, 6244287441641043454, -2520364922334079135),
        (2745456443212946947, 645118204916546775, 2745456443212946947),
        (7547064653499572189, 8045852435357409504, 7547064653499572189),
        (6854150699434287647, -4928293629714695488, 6854150699434287647),
        (-3287823213913993216, 436664784829482094, -3287823213913993216),
        (8148896601572644069, 6527982846972717980, 8148896601572644069),
        (7863550290393428639, -9099417587965344430, 7863550290393428639),
        (-3403909193876725561, -2326095921599201060, -3403909193876725561),
        (-8589464981105277044, -7562528811209179610, -8589464981105277044),
        (-8723573832670611078, -1328596429754306094, -8723573832670611078),
        (-5428641103648768818, 5541865167496835544, -5428641103648768818),
        (5577148701721471040, -1082373503339953000, 5577148701721471040),
        (3384500728989375414, -1840571093220315056, 3384500728989375414),
        (3669575232173215136, -6572753897029347230, 3669575232173215136),
        (-8523264075867997916, 1685698993337789708, -8523264075867997916),
        (-4568035919612258944, -8612573737953211128, -4568035919612258944),
        (-5308878537937002978, 361, -4568035919612258944),
    ];
}
