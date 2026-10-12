use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec as component_codec;

#[derive(Clone, Debug, PartialEq)]
pub enum HolderSet {
    Direct(Vec<i32>),
    Named(String, &'static [i32]),
}

impl HolderSet {
    pub fn decode(registry: &'static str, tag: &Nbt) -> DataResult<HolderSet> {
        if let Nbt::String(s) = tag
            && let Some(tag_name) = s.strip_prefix('#')
        {
            let key = crate::resources::identifier::read(tag_name)?;
            if !component_codec::tag_exists(registry, &key) {
                return Err(format!("Missing tag: '{key}' in '{registry}'"));
            }
            let contents = crisp_data::tag(registry, &key);
            return Ok(HolderSet::Named(key, contents));
        }
        let ids = codec::compact_list_of(tag, |element| {
            let key = codec::identifier(element)?;
            component_codec::registry_id(registry, &key)
        })?;
        Ok(HolderSet::Direct(ids))
    }

    pub fn contents(&self) -> &[i32] {
        match self {
            HolderSet::Direct(ids) => ids,
            HolderSet::Named(_, ids) => ids,
        }
    }

    pub fn contains(&self, id: i32) -> bool {
        self.contents().contains(&id)
    }
}
