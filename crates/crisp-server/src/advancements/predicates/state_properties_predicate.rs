use std::cmp::Ordering;

use crisp_data::{BlockState, FluidState};
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};

#[derive(Debug)]
enum ValueMatcher {
    Exact(String),
    Ranged(Option<String>, Option<String>),
}

impl ValueMatcher {
    fn decode(tag: &Nbt) -> DataResult<ValueMatcher> {
        match codec::either(tag, |t| Ok(codec::string(t)?.to_string()), |t| {
            codec::map(t)?;
            let bound = |key| codec::decode_optional(t, key, |v| Ok(codec::string(v)?.to_string()));
            Ok((bound("min")?, bound("max")?))
        })? {
            Ok(value) => Ok(ValueMatcher::Exact(value)),
            Err((min, max)) => Ok(ValueMatcher::Ranged(min, max)),
        }
    }

    fn matches(&self, values: &[&str], actual: &str) -> bool {
        match self {
            ValueMatcher::Exact(value) => compare(values, actual, value) == Some(Ordering::Equal),
            ValueMatcher::Ranged(min, max) => {
                if let Some(min) = min
                    && compare(values, actual, min).is_none_or(|o| o == Ordering::Less)
                {
                    return false;
                }
                if let Some(max) = max
                    && compare(values, actual, max).is_none_or(|o| o == Ordering::Greater)
                {
                    return false;
                }
                true
            }
        }
    }
}

fn compare(values: &[&str], actual: &str, expected: &str) -> Option<Ordering> {
    let expected_index = values.iter().position(|v| *v == expected)?;
    let actual_index = values.iter().position(|v| *v == actual)?;
    if let (Ok(a), Ok(e)) = (actual.parse::<i32>(), expected.parse::<i32>()) {
        return Some(a.cmp(&e));
    }
    if let (Ok(a), Ok(e)) = (actual.parse::<bool>(), expected.parse::<bool>()) {
        return Some(a.cmp(&e));
    }
    Some(actual_index.cmp(&expected_index))
}

#[derive(Debug)]
struct PropertyMatcher {
    name: String,
    value_matcher: ValueMatcher,
}

#[derive(Debug)]
pub struct StatePropertiesPredicate {
    properties: Vec<PropertyMatcher>,
}

impl StatePropertiesPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<StatePropertiesPredicate> {
        let properties = codec::unbounded_map(tag, |k| Ok(k.to_string()), ValueMatcher::decode)?
            .into_iter()
            .map(|(name, value_matcher)| PropertyMatcher { name, value_matcher })
            .collect();
        Ok(StatePropertiesPredicate { properties })
    }

    pub fn matches_block(&self, state: BlockState) -> bool {
        self.properties.iter().all(|matcher| {
            let Some((_, values)) = state.block().property_by_name(&matcher.name) else { return false };
            let actual = state.property_value_by_name(&matcher.name).expect("property of the block");
            matcher.value_matcher.matches(values, actual)
        })
    }

    pub fn matches_fluid(&self, state: FluidState) -> bool {
        self.properties.iter().all(|matcher| {
            let (values, actual): (&[&str], String) = match matcher.name.as_str() {
                "falling" if !state.is_empty() => (&["true", "false"], state.falling.to_string()),
                "level" if !state.is_empty() && !state.is_source() => (&["1", "2", "3", "4", "5", "6", "7", "8"], state.amount.to_string()),
                _ => return false,
            };
            matcher.value_matcher.matches(values, &actual)
        })
    }
}
