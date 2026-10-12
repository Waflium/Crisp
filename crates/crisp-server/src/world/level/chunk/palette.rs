use std::collections::HashMap;
use std::hash::Hash;

use crisp_core::bit_storage::{BitStorage, SimpleBitStorage};
use crisp_core::mth;
use crisp_protocol::Writer;

pub trait PaletteValue: Copy + Eq + Hash {
    fn global_id(self) -> u32;
    fn from_global_id(id: u32) -> Option<Self>;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Strategy {
    bits_per_axis: u32,
    biomes: bool,
    global_bits_in_memory: u32,
}

impl Strategy {
    pub fn block_states(global_size: usize) -> Strategy {
        Strategy { bits_per_axis: 4, biomes: false, global_bits_in_memory: mth::ceillog2(global_size as i32) as u32 }
    }

    pub fn biomes(global_size: usize) -> Strategy {
        Strategy { bits_per_axis: 2, biomes: true, global_bits_in_memory: mth::ceillog2(global_size as i32) as u32 }
    }

    pub fn entry_count(self) -> usize {
        1 << (self.bits_per_axis * 3)
    }

    pub fn index(self, x: usize, y: usize, z: usize) -> usize {
        (y << self.bits_per_axis | z) << self.bits_per_axis | x
    }

    fn configuration_for_bit_count(self, bits: u32) -> Configuration {
        use PaletteKind::*;
        let simple = |kind, bits| Configuration { kind, bits_in_memory: bits, bits_in_storage: bits };
        match (self.biomes, bits) {
            (_, 0) => simple(SingleValue, 0),
            (false, 1..=4) => simple(Linear, 4),
            (false, 5..=8) => simple(HashMap, bits),
            (true, 1..=3) => simple(Linear, bits),
            _ => Configuration { kind: Global, bits_in_memory: self.global_bits_in_memory, bits_in_storage: bits },
        }
    }

    fn configuration_for_palette_size(self, size: usize) -> Configuration {
        self.configuration_for_bit_count(mth::ceillog2(size as i32) as u32)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PaletteKind {
    SingleValue,
    Linear,
    HashMap,
    Global,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Configuration {
    kind: PaletteKind,
    bits_in_memory: u32,
    bits_in_storage: u32,
}

impl Configuration {
    fn always_repack(self) -> bool {
        self.kind == PaletteKind::Global
    }

    fn create_palette<T: PaletteValue>(self, entries: &[T]) -> Palette<T> {
        match self.kind {
            PaletteKind::SingleValue => {
                assert!(entries.len() <= 1, "Can't initialize SingleValuePalette with {} values.", entries.len());
                Palette::Single(entries.first().copied())
            }
            PaletteKind::Linear => {
                assert!(entries.len() <= 1 << self.bits_in_memory);
                Palette::Linear { values: entries.to_vec(), bits: self.bits_in_memory }
            }
            PaletteKind::HashMap => Palette::hash_map(self.bits_in_memory, entries),
            PaletteKind::Global => Palette::Global,
        }
    }
}

#[derive(Clone, Debug)]
enum Palette<T: PaletteValue> {
    Single(Option<T>),
    Linear { values: Vec<T>, bits: u32 },
    HashMap { values: Vec<T>, ids: HashMap<T, u32>, bits: u32 },
    Global,
}

enum IdFor {
    Id(u32),
    Resize(u32),
}

impl<T: PaletteValue> Palette<T> {
    fn hash_map(bits: u32, entries: &[T]) -> Palette<T> {
        let mut palette = Palette::HashMap { values: Vec::new(), ids: HashMap::new(), bits };
        for &e in entries {
            palette.add(e);
        }
        palette
    }

    fn add(&mut self, value: T) -> u32 {
        let Palette::HashMap { values, ids, .. } = self else { unreachable!() };
        let id = values.len() as u32;
        values.push(value);
        ids.insert(value, id);
        id
    }

    fn id_for(&mut self, value: T) -> IdFor {
        match self {
            Palette::Single(current) => match current {
                Some(v) if *v != value => IdFor::Resize(1),
                _ => {
                    *current = Some(value);
                    IdFor::Id(0)
                }
            },
            Palette::Linear { values, bits } => {
                if let Some(i) = values.iter().position(|v| *v == value) {
                    IdFor::Id(i as u32)
                } else if values.len() < 1 << *bits {
                    values.push(value);
                    IdFor::Id(values.len() as u32 - 1)
                } else {
                    IdFor::Resize(*bits + 1)
                }
            }
            Palette::HashMap { ids, bits, .. } => {
                if let Some(&id) = ids.get(&value) {
                    return IdFor::Id(id);
                }
                let bits = *bits;
                let id = self.add(value);
                if id >= 1 << bits { IdFor::Resize(bits + 1) } else { IdFor::Id(id) }
            }
            Palette::Global => IdFor::Id(value.global_id()),
        }
    }

    fn id_for_no_resize(&mut self, value: T) -> u32 {
        match self.id_for(value) {
            IdFor::Id(id) => id,
            IdFor::Resize(_) => panic!("Unexpected palette resize"),
        }
    }

    fn value_for(&self, id: u32) -> T {
        let found = match self {
            Palette::Single(v) => v.filter(|_| id == 0),
            Palette::Linear { values, .. } | Palette::HashMap { values, .. } => values.get(id as usize).copied(),
            Palette::Global => T::from_global_id(id),
        };
        found.unwrap_or_else(|| panic!("Missing Palette entry for index {id}."))
    }

    fn size(&self) -> usize {
        match self {
            Palette::Single(_) => 1,
            Palette::Linear { values, .. } | Palette::HashMap { values, .. } => values.len(),
            Palette::Global => usize::MAX,
        }
    }

    fn entries(&self) -> &[T] {
        match self {
            Palette::Linear { values, .. } | Palette::HashMap { values, .. } => values,
            Palette::Single(Some(v)) => std::slice::from_ref(v),
            _ => &[],
        }
    }

    fn write(&self, w: &mut Writer) {
        match self {
            Palette::Single(v) => w.varint(v.expect("Use of an uninitialized palette").global_id() as i32),
            Palette::Linear { values, .. } | Palette::HashMap { values, .. } => {
                w.varint(values.len() as i32);
                values.iter().for_each(|v| w.varint(v.global_id() as i32));
            }
            Palette::Global => {}
        }
    }

    fn maybe_has(&self, predicate: impl Fn(T) -> bool) -> bool {
        match self {
            Palette::Global => true,
            other => other.entries().iter().any(|&v| predicate(v)),
        }
    }
}

#[derive(Clone, Debug)]
struct Data<T: PaletteValue> {
    configuration: Configuration,
    storage: BitStorage,
    palette: Palette<T>,
}

impl<T: PaletteValue> Data<T> {
    fn copy_from(&mut self, old_palette: &Palette<T>, old_storage: &BitStorage) {
        if old_palette.size() == 1 {
            let id = self.palette.id_for_no_resize(old_palette.value_for(0));
            self.storage.fill(id);
        } else {
            let mut last_old = u32::MAX;
            let mut new_id = 0;
            for i in 0..old_storage.size() {
                let old_id = old_storage.get(i);
                if old_id != last_old {
                    new_id = self.palette.id_for_no_resize(old_palette.value_for(old_id));
                    last_old = old_id;
                }
                self.storage.set(i, new_id);
            }
        }
    }
}

pub struct PackedData<T> {
    pub palette: Vec<T>,
    pub storage: Option<Vec<i64>>,
}

#[derive(Clone, Debug)]
pub struct PalettedContainer<T: PaletteValue> {
    strategy: Strategy,
    data: Data<T>,
}

impl<T: PaletteValue> PalettedContainer<T> {
    pub fn new(initial: T, strategy: Strategy) -> PalettedContainer<T> {
        let mut container = PalettedContainer { strategy, data: Self::create_data(strategy, None, 0) };
        container.data.palette.id_for_no_resize(initial);
        container
    }

    fn create_data(strategy: Strategy, old: Option<&Data<T>>, bits: u32) -> Data<T> {
        let configuration = strategy.configuration_for_bit_count(bits);
        if let Some(old) = old.filter(|o| o.configuration == configuration) {
            return old.clone();
        }
        Data { configuration, storage: BitStorage::new(configuration.bits_in_memory, strategy.entry_count()), palette: configuration.create_palette(&[]) }
    }

    fn on_resize(&mut self, bits: u32, last_added: T) -> u32 {
        let mut new = Self::create_data(self.strategy, Some(&self.data), bits);
        new.copy_from(&self.data.palette, &self.data.storage);
        self.data = new;
        self.data.palette.id_for_no_resize(last_added)
    }

    fn id_for(&mut self, value: T) -> u32 {
        match self.data.palette.id_for(value) {
            IdFor::Id(id) => id,
            IdFor::Resize(bits) => self.on_resize(bits, value),
        }
    }

    pub fn recreate(&self) -> PalettedContainer<T> {
        PalettedContainer::new(self.data.palette.value_for(0), self.strategy)
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> T {
        self.get_index(self.strategy.index(x, y, z))
    }

    fn get_index(&self, index: usize) -> T {
        self.data.palette.value_for(self.data.storage.get(index))
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, value: T) {
        let index = self.strategy.index(x, y, z);
        let id = self.id_for(value);
        self.data.storage.set(index, id);
    }

    pub fn get_and_set(&mut self, x: usize, y: usize, z: usize, value: T) -> T {
        let index = self.strategy.index(x, y, z);
        let id = self.id_for(value);
        let old = self.data.storage.get_and_set(index, id);
        self.data.palette.value_for(old)
    }

    pub fn maybe_has(&self, predicate: impl Fn(T) -> bool) -> bool {
        self.data.palette.maybe_has(predicate)
    }

    pub fn count(&self) -> Vec<(T, usize)> {
        if self.data.palette.size() == 1 {
            return vec![(self.data.palette.value_for(0), self.data.storage.size())];
        }
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for i in 0..self.data.storage.size() {
            *counts.entry(self.data.storage.get(i)).or_default() += 1;
        }
        counts.into_iter().map(|(id, n)| (self.data.palette.value_for(id), n)).collect()
    }

    pub fn write(&self, w: &mut Writer) {
        w.u8(self.data.storage.bits() as u8);
        self.data.palette.write(w);
        self.data.storage.raw().iter().for_each(|&l| w.i64(l));
    }

    pub fn pack(&self) -> PackedData<T> {
        let mut palette = Palette::hash_map(self.data.storage.bits(), &[]);
        let contents = reencode(&self.data.storage, &self.data.palette, &mut palette);
        let entries = palette.entries().to_vec();
        let bits = self.strategy.configuration_for_palette_size(entries.len()).bits_in_storage;
        let storage = (bits != 0).then(|| SimpleBitStorage::from_values(bits, &contents).raw().to_vec());
        PackedData { palette: entries, storage }
    }

    pub fn unpack(strategy: Strategy, packed: PackedData<T>) -> Result<PalettedContainer<T>, String> {
        let entries = packed.palette;
        let count = strategy.entry_count();
        let configuration = strategy.configuration_for_palette_size(entries.len());
        let bits_on_disc = configuration.bits_in_storage;
        let (storage, palette) = if configuration.bits_in_memory == 0 {
            (BitStorage::Zero { size: count }, configuration.create_palette(&entries))
        } else {
            let data = packed.storage.ok_or("Missing values for non-zero storage")?;
            if !configuration.always_repack() && configuration.bits_in_memory == bits_on_disc {
                let storage = SimpleBitStorage::with_data(bits_on_disc, count, data).ok_or("Invalid length given for storage")?;
                (BitStorage::Simple(storage), configuration.create_palette(&entries))
            } else {
                let old_palette = Palette::hash_map(bits_on_disc, &entries);
                let old_storage = BitStorage::Simple(SimpleBitStorage::with_data(bits_on_disc, count, data).ok_or("Invalid length given for storage")?);
                let mut new_palette = configuration.create_palette(&entries);
                let contents = reencode(&old_storage, &old_palette, &mut new_palette);
                (BitStorage::Simple(SimpleBitStorage::from_values(configuration.bits_in_memory, &contents)), new_palette)
            }
        };
        Ok(PalettedContainer { strategy, data: Data { configuration, storage, palette } })
    }
}

fn reencode<T: PaletteValue>(storage: &BitStorage, old: &Palette<T>, new: &mut Palette<T>) -> Vec<u32> {
    let mut buffer = storage.unpack();
    let mut last_read = u32::MAX;
    let mut last_written = 0;
    for id in buffer.iter_mut() {
        if *id != last_read {
            last_read = *id;
            last_written = new.id_for_no_resize(old.value_for(*id));
        }
        *id = last_written;
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    struct V(u32);

    impl PaletteValue for V {
        fn global_id(self) -> u32 {
            self.0
        }
        fn from_global_id(id: u32) -> Option<V> {
            Some(V(id))
        }
    }

    #[test]
    fn grows_through_every_palette() {
        let strategy = Strategy::block_states(30000);
        let mut c = PalettedContainer::new(V(0), strategy);
        assert_eq!(c.data.storage.bits(), 0);
        c.set(0, 0, 0, V(7));
        assert_eq!((c.data.configuration.kind, c.data.storage.bits()), (PaletteKind::Linear, 4));
        for i in 1..20 {
            c.set(i, 1, 0, V(100 + i as u32));
        }
        assert_eq!((c.data.configuration.kind, c.data.storage.bits()), (PaletteKind::HashMap, 5));
        for i in 0..300u32 {
            let i = i as usize;
            c.set(i % 16, 2 + i / 256, i / 16 % 16, V(1000 + i as u32));
        }
        assert_eq!(c.data.configuration.kind, PaletteKind::Global);
        assert_eq!(c.data.storage.bits(), 15);
        assert_eq!(c.get(0, 0, 0), V(7));
        assert_eq!(c.get(3, 1, 0), V(103));

        let packed = c.pack();
        let read = PalettedContainer::unpack(strategy, packed).unwrap();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    assert_eq!(read.get(x, y, z), c.get(x, y, z));
                }
            }
        }
    }

    #[test]
    fn palette_keeps_insertion_order() {
        let mut c = PalettedContainer::new(V(0), Strategy::block_states(30000));
        for (i, v) in [79, 10, 9].into_iter().enumerate() {
            c.set(i, 0, 0, V(v));
        }
        assert_eq!(c.data.palette.entries(), &[V(0), V(79), V(10), V(9)]);
    }
}
