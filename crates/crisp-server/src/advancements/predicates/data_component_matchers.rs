use crisp_protocol::nbt::Nbt;

use crate::core::component::data_component_exact_predicate::DataComponentExactPredicate;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::core::component::predicates::data_component_predicate::DataComponentPredicate;
use crate::util::codec::{self, DataResult};

#[derive(Debug, Default)]
pub struct DataComponentMatchers {
    pub exact: DataComponentExactPredicate,
    pub partial: Vec<DataComponentPredicate>,
}

impl DataComponentMatchers {
    pub fn decode(tag: &Nbt) -> DataResult<DataComponentMatchers> {
        Ok(DataComponentMatchers {
            exact: codec::decode_or(tag, "components", DataComponentExactPredicate::default(), DataComponentExactPredicate::decode)?,
            partial: codec::decode_or(tag, "predicates", Vec::new(), DataComponentPredicate::decode_map)?,
        })
    }

    pub fn test(&self, values: &(impl DataComponentGetter + ?Sized)) -> bool {
        self.exact.test(values) && self.partial.iter().all(|predicate| predicate.matches(values))
    }

    pub fn is_empty(&self) -> bool {
        self.exact.is_empty() && self.partial.is_empty()
    }
}
