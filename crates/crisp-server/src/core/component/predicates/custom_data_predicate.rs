use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::nbt_predicate::NbtPredicate;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::DataResult;

#[derive(Debug)]
pub struct CustomDataPredicate {
    value: NbtPredicate,
}

impl CustomDataPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<CustomDataPredicate> {
        Ok(CustomDataPredicate { value: NbtPredicate::decode(tag)? })
    }

    pub fn matches(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        self.value.matches_components(components)
    }
}
