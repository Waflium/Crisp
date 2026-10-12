pub mod floats;
pub mod ints;
pub mod stored_number_access;

use crisp_protocol::nbt::Nbt;

use crate::core::holder::{Holder, RegistryElement};
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Debug)]
pub struct ArithmeticException(pub String);

impl ArithmeticException {
    pub fn new(message: impl Into<String>) -> ArithmeticException {
        ArithmeticException(message.into())
    }

    pub fn get_message(&self) -> &str {
        &self.0
    }
}

pub const INTEGER_OVERFLOW: &str = "integer overflow";
pub const DIVISION_BY_ZERO: &str = "/ by zero";

pub type ArithmeticResult<T> = Result<T, ArithmeticException>;

pub struct Case<V: RegistryElement> {
    condition: Holder<LootItemCondition>,
    pub value: Holder<V>,
}

impl<V: RegistryElement> Case<V> {
    pub fn decode(tag: &Nbt, value: fn(&Nbt) -> DataResult<Holder<V>>) -> DataResult<Case<V>> {
        Ok(Case { condition: codec::decode_field(tag, "condition", LootItemCondition::decode_holder)?, value: codec::decode_field(tag, "value", value)? })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.condition.value().test(context)
    }
}

pub fn decode_holder_list<V: RegistryElement>(tag: &Nbt, holder: fn(&Nbt) -> DataResult<Holder<V>>) -> DataResult<Vec<Holder<V>>> {
    if let Nbt::String(s) = tag
        && s.starts_with('#')
    {
        return Err(format!("Missing tag: '{}' in '{}'", &s[1..], V::REGISTRY));
    }
    codec::compact_list_of(tag, holder)
}

pub fn decode_weighted_list<V: RegistryElement>(tag: &Nbt, holder: fn(&Nbt) -> DataResult<Holder<V>>) -> DataResult<Vec<(Holder<V>, i32)>> {
    let entries = codec::list_of(tag, |entry| {
        let weight = codec::decode_field(entry, "weight", codec::int)?;
        if weight < 0 {
            return Err(format!("Value must be non-negative: {weight}"));
        }
        Ok((codec::decode_field(entry, "data", holder)?, weight))
    })?;
    if entries.is_empty() {
        return Err("List must have contents".into());
    }
    Ok(entries)
}

pub fn get_random<'v, V>(distribution: &'v [(V, i32)], context: &mut LootContext) -> &'v V {
    use crisp_core::random::RandomSource;
    let total_weight: i32 = distribution.iter().map(|(_, weight)| *weight).sum();
    let mut selection = context.random().next_int_bounded(total_weight);
    for (value, weight) in distribution {
        selection -= weight;
        if selection < 0 {
            return value;
        }
    }
    unreachable!("selection within the total weight")
}
