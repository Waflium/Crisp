use crisp_data::Item;
use anyhow::bail;
use crisp_protocol::Reader;

use crate::world::item::ItemStack;
use crate::world::item::component::{DataComponentPatch, DataComponentType};

const MAX_SIZE: i32 = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct HashedPatchMap {
    pub added_components: Vec<(DataComponentType, i32)>,
    pub removed_components: Vec<DataComponentType>,
}

pub fn hash_component(ty: DataComponentType, value: &crisp_protocol::nbt::Nbt) -> i32 {
    ty.hash(value) as i32
}

fn read_type(r: &mut Reader) -> anyhow::Result<DataComponentType> {
    let id = r.varint()?;
    DataComponentType::by_id(id).ok_or_else(|| anyhow::anyhow!("unknown data component type {id}"))
}

fn read_count(r: &mut Reader) -> anyhow::Result<i32> {
    let n = r.varint()?;
    if n > MAX_SIZE {
        bail!("{n} elements exceeded max size of: {MAX_SIZE}");
    }
    Ok(n.max(0))
}

impl HashedPatchMap {
    pub fn read(r: &mut Reader) -> anyhow::Result<HashedPatchMap> {
        let mut added_components: Vec<(DataComponentType, i32)> = Vec::new();
        for _ in 0..read_count(r)? {
            let ty = read_type(r)?;
            let hash = r.i32()?;
            added_components.retain(|(t, _)| *t != ty);
            added_components.push((ty, hash));
        }
        let mut removed_components = Vec::new();
        for _ in 0..read_count(r)? {
            let ty = read_type(r)?;
            if !removed_components.contains(&ty) {
                removed_components.push(ty);
            }
        }
        Ok(HashedPatchMap { added_components, removed_components })
    }

    #[cfg(test)]
    pub fn write(&self, w: &mut crisp_protocol::Writer) {
        w.varint(self.added_components.len() as i32);
        for (ty, hash) in &self.added_components {
            w.varint(ty.0 as i32);
            w.i32(*hash);
        }
        w.varint(self.removed_components.len() as i32);
        for ty in &self.removed_components {
            w.varint(ty.0 as i32);
        }
    }

    #[cfg(test)]
    pub fn create(patch: &DataComponentPatch) -> HashedPatchMap {
        let (added, removed) = patch.split();
        HashedPatchMap { added_components: added.into_iter().map(|(t, v)| (t, hash_component(t, v))).collect(), removed_components: removed }
    }

    pub fn matches(&self, patch: &DataComponentPatch) -> bool {
        let (added, removed) = patch.split();
        if removed.len() != self.removed_components.len() || !removed.iter().all(|t| self.removed_components.contains(t)) {
            return false;
        }
        if self.added_components.len() != added.len() {
            return false;
        }
        added.into_iter().all(|(ty, value)| {
            self.added_components.iter().find(|(t, _)| *t == ty).is_some_and(|(_, expected)| !ty.is_transient() && hash_component(ty, value) == *expected)
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HashedStack {
    Empty,
    ActualItem { item: Item, count: i32, components: HashedPatchMap },
}

impl HashedStack {
    pub fn read(r: &mut Reader) -> anyhow::Result<HashedStack> {
        if !r.bool()? {
            return Ok(HashedStack::Empty);
        }
        let id = r.varint()?;
        if id < 0 || id as usize >= Item::count() {
            bail!("unknown item id {id}");
        }
        let count = r.varint()?;
        let components = HashedPatchMap::read(r)?;
        Ok(HashedStack::ActualItem { item: Item(id as u16), count, components })
    }

    #[cfg(test)]
    pub fn write(&self, w: &mut crisp_protocol::Writer) {
        match self {
            HashedStack::Empty => w.bool(false),
            HashedStack::ActualItem { item, count, components } => {
                w.bool(true);
                w.varint(item.0 as i32);
                w.varint(*count);
                components.write(w);
            }
        }
    }

    #[cfg(test)]
    pub fn create(stack: Option<&ItemStack>) -> HashedStack {
        match stack {
            Some(s) if !s.is_empty() => HashedStack::ActualItem { item: s.item, count: s.count, components: HashedPatchMap::create(s.components_patch()) },
            _ => HashedStack::Empty,
        }
    }

    pub fn matches(&self, stack: Option<&ItemStack>) -> bool {
        let stack = stack.filter(|s| !s.is_empty());
        match (self, stack) {
            (HashedStack::Empty, None) => true,
            (HashedStack::Empty, Some(_)) | (HashedStack::ActualItem { .. }, None) => false,
            (HashedStack::ActualItem { item, count, components }, Some(s)) => *count == s.count && *item == s.item && components.matches(s.components_patch()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crisp_data::items;
    use crisp_protocol::nbt::Nbt;

    use crate::world::item::component as dc;

    #[test]
    fn claims_match_only_the_same_components() {
        let mut sword = ItemStack::new(items::DIAMOND_SWORD, 1);
        sword.set(dc::DAMAGE, Some(Nbt::Int(5)));
        sword.remove(dc::LORE);
        let mut w = crisp_protocol::Writer::new();
        HashedStack::create(Some(&sword)).write(&mut w);
        let claim = HashedStack::read(&mut Reader::new(w.as_slice())).unwrap();
        assert!(claim.matches(Some(&sword)));
        let mut other = sword.clone();
        other.set(dc::DAMAGE, Some(Nbt::Int(6)));
        assert!(!claim.matches(Some(&other)));
        assert!(!claim.matches(Some(&ItemStack::new(items::DIAMOND_SWORD, 1))));
        assert!(!claim.matches(None));
        assert!(HashedStack::Empty.matches(None));
    }
}
