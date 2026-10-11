use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::nbt_path_argument::compare_nbt;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::DataResult;
use crate::world::item::component as dc;

#[derive(Clone, Debug)]
pub struct NbtPredicate {
    pub tag: Nbt,
}

impl NbtPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<NbtPredicate> {
        match tag {
            Nbt::String(snbt) => match crate::nbt::parse_fully(snbt) {
                Ok(tag @ Nbt::Compound(_)) => Ok(NbtPredicate { tag }),
                Ok(other) => Err(format!("Expected compound tag, got {}", crate::nbt::to_string(&other))),
                Err(e) => Err(e.get_message()),
            },
            Nbt::Compound(_) => Ok(NbtPredicate { tag: tag.clone() }),
            other => Err(format!("Not a compound tag: {}", crate::nbt::to_string(other))),
        }
    }

    pub fn matches_components(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        match components.get(dc::CUSTOM_DATA) {
            Some(data) => compare_nbt(&self.tag, Some(&data)),
            None => compare_nbt(&self.tag, Some(&Nbt::Compound(Vec::new()))),
        }
    }

    pub fn matches(&self, tag: Option<&Nbt>) -> bool {
        tag.is_some_and(|tag| compare_nbt(&self.tag, Some(tag)))
    }
}
