use crisp_protocol::nbt::Nbt;

use super::loot_context::LootContext;
use super::providers::number::ints::ContextIntProvider;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};

pub enum IntRangePredicate {
    Point(Holder<ContextIntProvider>),
    Line { min: Option<Holder<ContextIntProvider>>, max: Option<Holder<ContextIntProvider>> },
}

impl IntRangePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<IntRangePredicate> {
        match codec::either(tag, ContextIntProvider::decode_holder, |t| {
            Ok((codec::decode_optional(t, "min", ContextIntProvider::decode_holder)?, codec::decode_optional(t, "max", ContextIntProvider::decode_holder)?))
        })? {
            Ok(point) => Ok(IntRangePredicate::Point(point)),
            Err((min, max)) => Ok(IntRangePredicate::Line { min, max }),
        }
    }

    pub fn test(&self, context: &mut LootContext, input: i32) -> bool {
        match self {
            IntRangePredicate::Point(value) => value.value().get_int(context) == input,
            IntRangePredicate::Line { min, max } => {
                if let Some(min) = min
                    && input < min.value().get_int(context)
                {
                    return false;
                }
                max.as_ref().is_none_or(|max| input <= max.value().get_int(context))
            }
        }
    }

    pub fn test_provider(&self, context: &mut LootContext, input: &ContextIntProvider) -> bool {
        match self {
            IntRangePredicate::Line { min: None, max: None } => true,
            _ => {
                let input = input.get_int(context);
                self.test(context, input)
            }
        }
    }
}
