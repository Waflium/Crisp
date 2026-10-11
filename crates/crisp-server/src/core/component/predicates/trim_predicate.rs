use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::registry_id;

fn holder_in(set: &HolderSet, registry: &str, holder: Option<&Nbt>) -> bool {
    holder.and_then(Nbt::as_str).and_then(|id| registry_id(registry, id).ok()).is_some_and(|id| set.contains(id))
}

#[derive(Debug)]
pub struct TrimPredicate {
    material: Option<HolderSet>,
    pattern: Option<HolderSet>,
}

impl TrimPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<TrimPredicate> {
        codec::map(tag)?;
        Ok(TrimPredicate {
            material: codec::decode_optional(tag, "material", |t| HolderSet::decode("minecraft:trim_material", t))?,
            pattern: codec::decode_optional(tag, "pattern", |t| HolderSet::decode("minecraft:trim_pattern", t))?,
        })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        if self.material.as_ref().is_some_and(|material| !holder_in(material, "minecraft:trim_material", value.get("material"))) {
            return false;
        }
        self.pattern.as_ref().is_none_or(|pattern| holder_in(pattern, "minecraft:trim_pattern", value.get("pattern")))
    }
}
