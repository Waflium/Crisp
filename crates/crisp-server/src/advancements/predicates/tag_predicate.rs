use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub struct TagPredicate {
    tag: HolderSet,
    expected: bool,
}

impl TagPredicate {
    pub fn decode(registry: &'static str, tag: &Nbt) -> DataResult<TagPredicate> {
        Ok(TagPredicate { tag: codec::decode_field(tag, "id", |t| HolderSet::decode(registry, t))?, expected: codec::decode_field(tag, "expected", codec::bool_value)? })
    }

    pub fn matches(&self, id: i32) -> bool {
        self.tag.contains(id) == self.expected
    }
}
