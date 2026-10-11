use crisp_protocol::nbt::Nbt;

use super::loot_context::LootContext;
use super::providers::number::ints::ContextIntProvider;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};

pub struct IntLimit {
    min: Option<Holder<ContextIntProvider>>,
    max: Option<Holder<ContextIntProvider>>,
}

impl IntLimit {
    pub fn decode(tag: &Nbt) -> DataResult<IntLimit> {
        codec::map(tag)?;
        Ok(IntLimit {
            min: codec::decode_optional(tag, "min", ContextIntProvider::decode_holder)?,
            max: codec::decode_optional(tag, "max", ContextIntProvider::decode_holder)?,
        })
    }

    pub fn clamp(&self, context: &mut LootContext, input: i32) -> i32 {
        match (&self.min, &self.max) {
            (None, None) => input,
            (None, Some(max)) => max.value().get_int(context).min(input),
            (Some(min), None) => min.value().get_int(context).max(input),
            (Some(min), Some(max)) => {
                let min = min.value().get_int(context);
                let max = max.value().get_int(context);
                if input < min { min } else { input.min(max) }
            }
        }
    }
}
