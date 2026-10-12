pub mod attack_range;
pub mod bundle_contents;
pub mod charged_projectiles;
pub mod codec;
pub(crate) mod custom;
pub mod growable_mutable_container;
pub mod item_container_contents;
pub mod map_post_processing;
pub mod simple_mutable_container;
#[cfg(test)]
mod oracle_data;
#[cfg(test)]
mod oracle_tests;
pub(crate) mod text;
pub(crate) mod types;
pub mod swing_animation;
pub mod values;

use std::sync::LazyLock;

use crisp_data::{Item, registry};
use crisp_protocol::nbt::{Nbt, java_hash_map_order};
use crisp_protocol::{Reader, Writer};

use crate::util::hash_ops::HashCode;
use codec::{Codec, Stream};

pub use custom::patch_of;
pub use types::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DataComponentType(pub u16);

pub struct TypeInfo {
    pub persistent: bool,
    pub codec: Codec,
    pub stream: Stream,
}

static TYPES: LazyLock<Vec<TypeInfo>> = LazyLock::new(types::build);

impl DataComponentType {
    pub fn name(self) -> &'static str {
        registry::DATA_COMPONENT_TYPE[self.0 as usize]
    }

    pub fn by_name(name: &str) -> Option<DataComponentType> {
        registry::DATA_COMPONENT_TYPE.iter().position(|&c| c == name).map(|i| DataComponentType(i as u16))
    }

    pub fn by_id(id: i32) -> Option<DataComponentType> {
        (0..registry::DATA_COMPONENT_TYPE.len() as i32).contains(&id).then_some(DataComponentType(id as u16))
    }

    pub fn info(self) -> &'static TypeInfo {
        &TYPES[self.0 as usize]
    }

    pub fn is_transient(self) -> bool {
        !self.info().persistent
    }

    pub fn decode(self, tag: &Nbt) -> Result<Nbt, String> {
        self.info().codec.decode(tag)
    }

    pub fn hash(self, value: &Nbt) -> HashCode {
        self.info().codec.hash(value)
    }

    pub fn write(self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let info = self.info();
        codec::write(&info.codec, &info.stream, value, w)
    }

    pub fn read(self, r: &mut Reader) -> Result<Nbt, String> {
        let info = self.info();
        codec::read(&info.codec, &info.stream, r)
    }
}

impl std::fmt::Debug for DataComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DataComponentMap {
    entries: Vec<(DataComponentType, Nbt)>,
}

impl DataComponentMap {
    pub const EMPTY: DataComponentMap = DataComponentMap { entries: Vec::new() };

    pub fn new(mut entries: Vec<(DataComponentType, Nbt)>) -> DataComponentMap {
        entries.sort_by_key(|(t, _)| *t);
        entries.dedup_by_key(|(t, _)| *t);
        DataComponentMap { entries }
    }

    pub fn get(&self, ty: DataComponentType) -> Option<&Nbt> {
        self.entries.binary_search_by_key(&ty, |(t, _)| *t).ok().map(|i| &self.entries[i].1)
    }

    pub fn has(&self, ty: DataComponentType) -> bool {
        self.get(ty).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (DataComponentType, &Nbt)> {
        self.entries.iter().map(|(t, v)| (*t, v))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub fn json_to_nbt(v: &serde_json::Value) -> Nbt {
    use serde_json::Value;
    match v {
        Value::Null => Nbt::Compound(Vec::new()),
        Value::Bool(b) => Nbt::Byte(*b as i8),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Nbt::Long(i),
            None => Nbt::Double(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => Nbt::String(s.clone()),
        Value::Array(items) => Nbt::List(items.iter().map(json_to_nbt).collect()),
        Value::Object(fields) => Nbt::Compound(fields.iter().map(|(k, v)| (k.clone(), json_to_nbt(v))).collect()),
    }
}

static ITEM_COMPONENTS: LazyLock<Vec<DataComponentMap>> = LazyLock::new(|| {
    crisp_data::ITEM_COMPONENTS
        .iter()
        .map(|json| {
            let value: serde_json::Value = serde_json::from_str(json).expect("item components report");
            let serde_json::Value::Object(fields) = value else { return DataComponentMap::EMPTY };
            let entries = fields
                .iter()
                .filter_map(|(id, v)| {
                    let ty = DataComponentType::by_name(id)?;
                    let tag = json_to_nbt(v);
                    let value = ty.decode(&tag).unwrap_or(tag);
                    Some((ty, value))
                })
                .collect();
            DataComponentMap::new(entries)
        })
        .collect()
});

pub fn item_components(item: Item) -> &'static DataComponentMap {
    static EMPTY: DataComponentMap = DataComponentMap::EMPTY;
    ITEM_COMPONENTS.get(item.0 as usize).unwrap_or(&EMPTY)
}

#[derive(Clone, Debug, Default)]
pub struct DataComponentPatch {
    entries: Vec<(DataComponentType, Option<Nbt>)>,
}

impl PartialEq for DataComponentPatch {
    fn eq(&self, other: &DataComponentPatch) -> bool {
        self.entries.len() == other.entries.len() && self.entries.iter().all(|(t, v)| other.get(*t) == Some(v.as_ref()))
    }
}

impl DataComponentPatch {
    pub const EMPTY: DataComponentPatch = DataComponentPatch { entries: Vec::new() };

    pub fn get(&self, ty: DataComponentType) -> Option<Option<&Nbt>> {
        self.entries.iter().find(|(t, _)| *t == ty).map(|(_, v)| v.as_ref())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (DataComponentType, Option<&Nbt>)> {
        self.entries.iter().map(|(t, v)| (*t, v.as_ref()))
    }

    pub fn put(&mut self, ty: DataComponentType, value: Option<Nbt>) {
        match self.entries.iter_mut().find(|(t, _)| *t == ty) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((ty, value)),
        }
    }

    fn remove_entry(&mut self, ty: DataComponentType) -> Option<Option<Nbt>> {
        let i = self.entries.iter().position(|(t, _)| *t == ty)?;
        Some(self.entries.remove(i).1)
    }

    pub fn split(&self) -> (Vec<(DataComponentType, &Nbt)>, Vec<DataComponentType>) {
        let added = self.entries.iter().filter_map(|(t, v)| v.as_ref().map(|v| (*t, v))).collect();
        let removed = self.entries.iter().filter(|(_, v)| v.is_none()).map(|(t, _)| *t).collect();
        (added, removed)
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(
            self.entries
                .iter()
                .filter(|(t, _)| !t.is_transient())
                .map(|(t, v)| match v {
                    Some(v) => (t.name().to_string(), v.clone()),
                    None => (format!("!{}", t.name()), Nbt::Compound(Vec::new())),
                })
                .collect(),
        )
    }

    pub fn from_nbt(tag: &Nbt) -> Result<DataComponentPatch, String> {
        let fields = codec::map(tag)?;
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        let mut patch = DataComponentPatch::default();
        let mut errors = Vec::new();
        for i in java_hash_map_order(&keys) {
            let (key, value) = &fields[i];
            let (removed, name) = match key.strip_prefix('!') {
                Some(name) => (true, name),
                None => (false, key.as_str()),
            };
            let Some(id) = crate::commands::arguments::parse_identifier(name) else {
                errors.push(format!("Not a valid resource location: {name}"));
                continue;
            };
            let ty = match DataComponentType::by_name(&id) {
                Some(ty) if ty.is_transient() => {
                    errors.push(format!("'{id}' is not a persistent component"));
                    continue;
                }
                Some(ty) => ty,
                None => {
                    errors.push(format!("No component with type: '{id}'"));
                    continue;
                }
            };
            if removed {
                match codec::map(value) {
                    Ok(_) => patch.put(ty, None),
                    Err(e) => errors.push(e),
                }
            } else {
                match ty.decode(value) {
                    Ok(v) => patch.put(ty, Some(v)),
                    Err(e) => errors.push(e),
                }
            }
        }
        if errors.is_empty() { Ok(patch) } else { Err(errors.join("; ")) }
    }

    pub fn write(&self, w: &mut Writer, delimited: bool) -> Result<(), String> {
        let positive = self.entries.iter().filter(|(_, v)| v.is_some()).count();
        w.varint(positive as i32);
        w.varint((self.entries.len() - positive) as i32);
        for (t, v) in &self.entries {
            if let Some(v) = v {
                w.varint(t.0 as i32);
                if delimited {
                    let mut inner = Writer::new();
                    t.write(v, &mut inner)?;
                    w.varint(inner.as_slice().len() as i32);
                    w.bytes(inner.as_slice());
                } else {
                    t.write(v, w)?;
                }
            }
        }
        for (t, v) in &self.entries {
            if v.is_none() {
                w.varint(t.0 as i32);
            }
        }
        Ok(())
    }

    pub fn read(r: &mut Reader, delimited: bool) -> Result<DataComponentPatch, String> {
        let positive = r.varint().map_err(|e| e.to_string())?;
        let negative = r.varint().map_err(|e| e.to_string())?;
        let mut patch = DataComponentPatch::default();
        let read_type = |r: &mut Reader| -> Result<DataComponentType, String> {
            let id = r.varint().map_err(|e| e.to_string())?;
            DataComponentType::by_id(id).ok_or_else(|| format!("Unknown data component type id {id}"))
        };
        for _ in 0..positive.max(0) {
            let ty = read_type(r)?;
            let value = if delimited {
                let size = r.varint().map_err(|e| e.to_string())?;
                let bytes = r.take(size.max(0) as usize).map_err(|e| e.to_string())?;
                let mut inner = Reader::new(bytes);
                ty.read(&mut inner)?
            } else {
                ty.read(r)?
            };
            patch.put(ty, Some(value));
        }
        for _ in 0..negative.max(0) {
            let ty = read_type(r)?;
            patch.put(ty, None);
        }
        Ok(patch)
    }
}

#[derive(Clone)]
pub struct PatchedDataComponentMap {
    prototype: &'static DataComponentMap,
    patch: DataComponentPatch,
}

impl std::fmt::Debug for PatchedDataComponentMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.patch.iter().map(|(t, v)| (t, v.map(crate::nbt::to_string)))).finish()
    }
}

impl PartialEq for PatchedDataComponentMap {
    fn eq(&self, other: &PatchedDataComponentMap) -> bool {
        std::ptr::eq(self.prototype, other.prototype) && self.patch == other.patch
    }
}

impl PatchedDataComponentMap {
    pub fn new(prototype: &'static DataComponentMap) -> PatchedDataComponentMap {
        PatchedDataComponentMap { prototype, patch: DataComponentPatch::default() }
    }

    pub fn from_patch(prototype: &'static DataComponentMap, patch: &DataComponentPatch) -> PatchedDataComponentMap {
        let mut map = PatchedDataComponentMap::new(prototype);
        map.apply_patch(patch);
        map
    }

    pub fn prototype(&self) -> &'static DataComponentMap {
        self.prototype
    }

    pub fn get(&self, ty: DataComponentType) -> Option<&Nbt> {
        match self.patch.get(ty) {
            Some(value) => value,
            None => self.prototype.get(ty),
        }
    }

    pub fn has(&self, ty: DataComponentType) -> bool {
        self.get(ty).is_some()
    }

    pub fn has_non_default(&self, ty: DataComponentType) -> bool {
        self.patch.get(ty).is_some()
    }

    pub fn set(&mut self, ty: DataComponentType, value: Option<Nbt>) -> Option<Nbt> {
        let default = self.prototype.get(ty);
        let last = if value.as_ref() == default { self.patch.remove_entry(ty) } else { replace(&mut self.patch, ty, value) };
        match last {
            Some(last) => last.or_else(|| default.cloned()),
            None => default.cloned(),
        }
    }

    pub fn remove(&mut self, ty: DataComponentType) -> Option<Nbt> {
        let default = self.prototype.get(ty);
        let last = if default.is_some() { replace(&mut self.patch, ty, None) } else { self.patch.remove_entry(ty) };
        match last {
            Some(last) => last,
            None => default.cloned(),
        }
    }

    pub fn apply_patch(&mut self, patch: &DataComponentPatch) {
        for (ty, value) in patch.iter() {
            let default = self.prototype.get(ty);
            match value {
                Some(value) if Some(value) == default => {
                    self.patch.remove_entry(ty);
                }
                Some(value) => self.patch.put(ty, Some(value.clone())),
                None if default.is_some() => self.patch.put(ty, None),
                None => {
                    self.patch.remove_entry(ty);
                }
            }
        }
    }

    pub fn restore_patch(&mut self, patch: DataComponentPatch) {
        self.patch = patch;
    }

    pub fn clear_patch(&mut self) {
        self.patch = DataComponentPatch::default();
    }

    pub fn as_patch(&self) -> &DataComponentPatch {
        &self.patch
    }

    pub fn size(&self) -> usize {
        let mut size = self.prototype.len() as isize;
        for (ty, value) in self.patch.iter() {
            let in_patch = value.is_some();
            if in_patch != self.prototype.has(ty) {
                size += if in_patch { 1 } else { -1 };
            }
        }
        size as usize
    }

    pub fn iter(&self) -> impl Iterator<Item = (DataComponentType, &Nbt)> {
        self.patch
            .iter()
            .filter_map(|(t, v)| v.map(|v| (t, v)))
            .chain(self.prototype.iter().filter(|(t, _)| self.patch.get(*t).is_none()))
    }
}

fn replace(patch: &mut DataComponentPatch, ty: DataComponentType, value: Option<Nbt>) -> Option<Option<Nbt>> {
    match patch.entries.iter_mut().find(|(t, _)| *t == ty) {
        Some(entry) => Some(std::mem::replace(&mut entry.1, value)),
        None => {
            patch.entries.push((ty, value));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_the_registry() {
        for (ty, name) in NAMED {
            assert_eq!(ty.name(), *name);
        }
        assert_eq!(TYPES.len(), registry::DATA_COMPONENT_TYPE.len());
    }

    #[test]
    fn defaults_are_canonical() {
        let sword = item_components(crisp_data::items::DIAMOND_SWORD);
        let tool = crate::nbt::parse_fully(
            "{rules:[{blocks:\"minecraft:cobweb\",correct_for_drops:1b,speed:15.0f},{blocks:\"#minecraft:sword_instantly_mines\",speed:3.4028235E38f},\
             {blocks:\"#minecraft:sword_efficient\",speed:1.5f}],damage_per_block:2,can_destroy_blocks_in_creative:0b}",
        )
        .unwrap();
        assert_eq!(sword.get(TOOL), Some(&tool));
        assert_eq!(sword.get(MAX_DAMAGE), Some(&Nbt::Int(1561)));
        assert_eq!(sword.get(ITEM_NAME), Some(&crate::nbt::parse_fully("{translate:\"item.minecraft.diamond_sword\"}").unwrap()));
        assert_eq!(sword.get(USE_EFFECTS), Some(&Nbt::Compound(Vec::new())));
        assert_eq!(sword.len(), 19);
    }

    fn same_as_json(nbt: &Nbt, json: &serde_json::Value) -> bool {
        use serde_json::Value;
        match (nbt, json) {
            (Nbt::Byte(b), Value::Bool(v)) => (*b != 0) == *v,
            (Nbt::Float(f), Value::Number(n)) => n.as_f64().is_some_and(|n| n as f32 == *f),
            (Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Double(_), Value::Number(n)) => n.as_f64() == Some(codec::as_f64(nbt)),
            (Nbt::String(a), Value::String(b)) => a == b,
            (Nbt::List(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_as_json(a, b)),
            (Nbt::IntArray(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| b.as_i64() == Some(*a as i64)),
            (Nbt::Compound(a), Value::Object(b)) => a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|b| same_as_json(v, b))),
            _ => false,
        }
    }

    #[test]
    fn every_item_default_round_trips() {
        let mut failures = Vec::new();
        for (index, json) in crisp_data::ITEM_COMPONENTS.iter().enumerate() {
            let serde_json::Value::Object(fields) = serde_json::from_str(json).unwrap() else { panic!() };
            for (id, v) in &fields {
                let ty = DataComponentType::by_name(id).unwrap();
                match ty.decode(&json_to_nbt(v)) {
                    Ok(value) if same_as_json(&value, v) => {}
                    Ok(value) => failures.push(format!("{:?} {id}: {} != {v}", Item(index as u16), crate::nbt::to_string(&value))),
                    Err(e) => failures.push(format!("{:?} {id}: {e}", Item(index as u16))),
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
