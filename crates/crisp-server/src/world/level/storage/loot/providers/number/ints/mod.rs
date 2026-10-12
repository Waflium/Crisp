pub mod resolvable_int;

use crisp_core::mth;
use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use super::floats::ContextFloatProvider;
use super::stored_number_access::StoredNumberAccess;
use super::{ArithmeticException, ArithmeticResult, Case, DIVISION_BY_ZERO, INTEGER_OVERFLOW, decode_holder_list, decode_weighted_list, get_random};
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DataResult};
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::providers::score::ScoreboardNameProvider;

pub enum ContextIntProvider {
    Absolute(Holder<ContextIntProvider>),
    Average(Vec<Holder<ContextIntProvider>>),
    BinomialDistributionGenerator { n: Holder<ContextIntProvider>, p: Holder<ContextFloatProvider> },
    ConditionalValue { condition: Holder<LootItemCondition>, on_true: Holder<ContextIntProvider>, on_false: Holder<ContextIntProvider> },
    ConstantValue(i32),
    Difference { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    EnvironmentAttributeValue(&'static EnvironmentAttributeInfo),
    FromFloat(Holder<ContextFloatProvider>),
    Maximum(Vec<Holder<ContextIntProvider>>),
    Minimum(Vec<Holder<ContextIntProvider>>),
    FloorModulus { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    FloorQuotient { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    Modulus { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    Quotient { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    Negate(Holder<ContextIntProvider>),
    NumberDispatcher { cases: Vec<Case<ContextIntProvider>>, default_value: Holder<ContextIntProvider> },
    Power { base: Holder<ContextIntProvider>, exponent: Holder<ContextIntProvider> },
    Product(Vec<Holder<ContextIntProvider>>),
    ScoreboardValue { target: ScoreboardNameProvider, score: String, fallback: Holder<ContextIntProvider> },
    StorageValue { access: StoredNumberAccess, fallback: Holder<ContextIntProvider> },
    Sum(Vec<Holder<ContextIntProvider>>),
    UniformGenerator { min: Holder<ContextIntProvider>, max: Holder<ContextIntProvider> },
    WeightedListValue(Vec<(Holder<ContextIntProvider>, i32)>),
}

impl RegistryElement for ContextIntProvider {
    const REGISTRY: &'static str = "minecraft:context_int_provider";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static ContextIntProvider> {
        reloadable_server_registries::get().context_int_providers.get(key)
    }
}

pub fn exactly(value: i32) -> Holder<ContextIntProvider> {
    Holder::direct(ContextIntProvider::ConstantValue(value))
}

fn long_to_int_safe(value: i64) -> ArithmeticResult<i32> {
    i32::try_from(value).map_err(|_| ArithmeticException::new(format!("Value {value} can't be safely converted to int")))
}

pub fn float_to_int_safe(value: f32) -> ArithmeticResult<i32> {
    if !value.is_finite() {
        return Err(ArithmeticException::new(format!("Value {} can't be safely converted to int", crate::nbt::java_float_to_string(value))));
    }
    long_to_int_safe(value as i64)
}

fn overflow() -> ArithmeticException {
    ArithmeticException::new(INTEGER_OVERFLOW)
}

fn division_by_zero() -> ArithmeticException {
    ArithmeticException::new(DIVISION_BY_ZERO)
}

impl ContextIntProvider {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<ContextIntProvider>> {
        Holder::decode(tag, ContextIntProvider::decode)
    }

    fn decode_holders(tag: &Nbt) -> DataResult<Vec<Holder<ContextIntProvider>>> {
        decode_holder_list(tag, ContextIntProvider::decode_holder)
    }

    pub fn decode(tag: &Nbt) -> DataResult<ContextIntProvider> {
        match codec::either(tag, codec::int, ContextIntProvider::decode_typed)? {
            Ok(value) => Ok(ContextIntProvider::ConstantValue(value)),
            Err(provider) => Ok(provider),
        }
    }

    fn decode_typed(tag: &Nbt) -> DataResult<ContextIntProvider> {
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        let holder = |key| codec::decode_field(tag, key, ContextIntProvider::decode_holder);
        let inputs = || codec::decode_field(tag, "inputs", ContextIntProvider::decode_holders);
        Ok(match ty.as_str() {
            "minecraft:abs" => ContextIntProvider::Absolute(holder("input")?),
            "minecraft:avg" => ContextIntProvider::Average(inputs()?),
            "minecraft:binomial" => ContextIntProvider::BinomialDistributionGenerator {
                n: holder("n")?,
                p: codec::decode_field(tag, "p", ContextFloatProvider::decode_holder)?,
            },
            "minecraft:conditional" => ContextIntProvider::ConditionalValue {
                condition: codec::decode_field(tag, "condition", LootItemCondition::decode_holder)?,
                on_true: holder("on_true")?,
                on_false: codec::decode_or(tag, "on_false", exactly(0), ContextIntProvider::decode_holder)?,
            },
            "minecraft:constant" => ContextIntProvider::ConstantValue(codec::decode_field(tag, "value", codec::int)?),
            "minecraft:sub" => ContextIntProvider::Difference { left: holder("left")?, right: holder("right")? },
            "minecraft:environment_attribute" => {
                let name = codec::decode_field(tag, "attribute", codec::identifier)?;
                let attribute = EnvironmentAttributeInfo::by_name(&name).ok_or_else(|| format!("Unknown registry key: {name}"))?;
                if attribute.ty.to_int(attribute.default_value()).is_none() {
                    return Err(format!("{name} cannot be converted to an integer"));
                }
                ContextIntProvider::EnvironmentAttributeValue(attribute)
            }
            "minecraft:from_float" => ContextIntProvider::FromFloat(codec::decode_field(tag, "input", ContextFloatProvider::decode_holder)?),
            "minecraft:max" => ContextIntProvider::Maximum(inputs()?),
            "minecraft:min" => ContextIntProvider::Minimum(inputs()?),
            "minecraft:floor_mod" => ContextIntProvider::FloorModulus { left: holder("left")?, right: holder("right")? },
            "minecraft:floor_div" => ContextIntProvider::FloorQuotient { left: holder("left")?, right: holder("right")? },
            "minecraft:mod" => ContextIntProvider::Modulus { left: holder("left")?, right: holder("right")? },
            "minecraft:div" => ContextIntProvider::Quotient { left: holder("left")?, right: holder("right")? },
            "minecraft:negate" => ContextIntProvider::Negate(holder("input")?),
            "minecraft:number_dispatcher" => ContextIntProvider::NumberDispatcher {
                cases: codec::decode_field(tag, "cases", |t| codec::list_of(t, |c| Case::decode(c, ContextIntProvider::decode_holder)))?,
                default_value: codec::decode_or(tag, "default", exactly(0), ContextIntProvider::decode_holder)?,
            },
            "minecraft:pow" => ContextIntProvider::Power { base: holder("base")?, exponent: holder("exponent")? },
            "minecraft:mul" => ContextIntProvider::Product(inputs()?),
            "minecraft:score" => ContextIntProvider::ScoreboardValue {
                target: codec::decode_field(tag, "target", ScoreboardNameProvider::decode)?,
                score: codec::decode_field(tag, "score", |t| Ok(codec::string(t)?.to_string()))?,
                fallback: codec::decode_or(tag, "fallback", exactly(0), ContextIntProvider::decode_holder)?,
            },
            "minecraft:storage" => ContextIntProvider::StorageValue {
                access: StoredNumberAccess::decode(tag)?,
                fallback: codec::decode_or(tag, "fallback", exactly(0), ContextIntProvider::decode_holder)?,
            },
            "minecraft:add" => ContextIntProvider::Sum(inputs()?),
            "minecraft:uniform" => ContextIntProvider::UniformGenerator { min: holder("min")?, max: holder("max")? },
            "minecraft:weighted_list" => ContextIntProvider::WeightedListValue(codec::decode_field(tag, "distribution", |t| {
                decode_weighted_list(t, ContextIntProvider::decode_holder)
            })?),
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:context_int_provider_type]: {other}")),
        })
    }

    pub fn get_int(&self, context: &mut LootContext) -> i32 {
        self.get_int_unsafe(context).unwrap_or(0)
    }

    pub fn get_int_unsafe(&self, context: &mut LootContext) -> ArithmeticResult<i32> {
        match self {
            ContextIntProvider::Absolute(input) => {
                input.value().get_int_unsafe(context)?.checked_abs().ok_or_else(|| ArithmeticException::new("Overflow to represent absolute value of Integer.MIN_VALUE"))
            }
            ContextIntProvider::Average(inputs) => {
                let (mut sum, mut count) = (0i64, 0i64);
                for input in inputs {
                    sum = sum.wrapping_add(input.value().get_int_unsafe(context)? as i64);
                    count += 1;
                }
                if count == 0 {
                    return Err(division_by_zero());
                }
                long_to_int_safe(sum / count)
            }
            ContextIntProvider::BinomialDistributionGenerator { n, p } => {
                let n = n.value().get_int_unsafe(context)?;
                let p = p.value().get_float_or_throw(context)?;
                let mut result = 0;
                for _ in 0..n {
                    if context.random().next_float() < p {
                        result += 1;
                    }
                }
                Ok(result)
            }
            ContextIntProvider::ConditionalValue { condition, on_true, on_false } => {
                let value = if condition.value().test(context) { on_true } else { on_false };
                value.value().get_int_unsafe(context)
            }
            ContextIntProvider::ConstantValue(value) => Ok(*value),
            ContextIntProvider::Difference { left, right } => {
                left.value().get_int_unsafe(context)?.checked_sub(right.value().get_int_unsafe(context)?).ok_or_else(overflow)
            }
            ContextIntProvider::EnvironmentAttributeValue(attribute) => {
                let value = context.environment_attribute_value(attribute);
                attribute.ty.to_int(&value).ok_or_else(|| ArithmeticException::new(format!("{value:?} cannot be represented as an integer")))
            }
            ContextIntProvider::FromFloat(input) => float_to_int_safe(input.value().get_float_unsafe(context)?),
            ContextIntProvider::Maximum(inputs) => {
                let mut value = i32::MIN;
                for input in inputs {
                    value = value.max(input.value().get_int_unsafe(context)?);
                }
                Ok(value)
            }
            ContextIntProvider::Minimum(inputs) => {
                let mut value = i32::MAX;
                for input in inputs {
                    value = value.min(input.value().get_int_unsafe(context)?);
                }
                Ok(value)
            }
            ContextIntProvider::FloorModulus { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                let modulus = left.wrapping_rem(right);
                Ok(if modulus != 0 && (modulus ^ right) < 0 { modulus + right } else { modulus })
            }
            ContextIntProvider::FloorQuotient { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                if left == i32::MIN && right == -1 {
                    return Err(overflow());
                }
                let quotient = left / right;
                Ok(if (left ^ right) < 0 && quotient * right != left { quotient - 1 } else { quotient })
            }
            ContextIntProvider::Modulus { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                Ok(left.wrapping_rem(right))
            }
            ContextIntProvider::Quotient { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                Ok(left.wrapping_div(right))
            }
            ContextIntProvider::Negate(input) => input.value().get_int_unsafe(context)?.checked_neg().ok_or_else(overflow),
            ContextIntProvider::NumberDispatcher { cases, default_value } => {
                for case in cases {
                    if case.test(context) {
                        return case.value.value().get_int_unsafe(context);
                    }
                }
                default_value.value().get_int_unsafe(context)
            }
            ContextIntProvider::Power { base, exponent } => {
                let (base, exponent) = (base.value().get_int_unsafe(context)?, exponent.value().get_int_unsafe(context)?);
                if base == 0 && exponent == 0 {
                    return Err(ArithmeticException::new("Result of 0 to the power of 0 is undefined"));
                }
                if exponent < 0 {
                    return Err(ArithmeticException::new("negative exponent"));
                }
                base.checked_pow(exponent as u32).ok_or_else(overflow)
            }
            ContextIntProvider::Product(inputs) => {
                let mut value = 1i64;
                for input in inputs {
                    value = value.wrapping_mul(input.value().get_int_unsafe(context)? as i64);
                }
                long_to_int_safe(value)
            }
            ContextIntProvider::ScoreboardValue { target, score, fallback } => match target.score_value(context, score) {
                Some(value) => Ok(value),
                None => fallback.value().get_int_unsafe(context),
            },
            ContextIntProvider::StorageValue { access, fallback } => match access.get_numeric_tag(context) {
                Some(value) => Ok(crate::world::item::component::codec::as_i32(&value)),
                None => fallback.value().get_int_unsafe(context),
            },
            ContextIntProvider::Sum(inputs) => {
                let mut value = 0i64;
                for input in inputs {
                    value = value.wrapping_add(input.value().get_int_unsafe(context)? as i64);
                }
                long_to_int_safe(value)
            }
            ContextIntProvider::UniformGenerator { min, max } => {
                let (min, max) = (min.value().get_int_unsafe(context)?, max.value().get_int_unsafe(context)?);
                Ok(mth::next_int(&mut context.random(),min, max))
            }
            ContextIntProvider::WeightedListValue(distribution) => get_random(distribution, context).value().get_int_unsafe(context),
        }
    }
}
