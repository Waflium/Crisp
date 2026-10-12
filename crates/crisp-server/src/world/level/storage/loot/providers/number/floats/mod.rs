pub mod resolvable_float;

use crisp_core::mth;
use crisp_protocol::nbt::Nbt;

use super::ints::ContextIntProvider;
use super::stored_number_access::StoredNumberAccess;
use super::{ArithmeticException, ArithmeticResult, Case, decode_holder_list, decode_weighted_list, get_random};
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DataResult};
use crate::util::{java_max_f, java_min_f};
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::item::enchantment::level_based_value::LevelBasedValue;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

pub enum ContextFloatProvider {
    Absolute(Holder<ContextFloatProvider>),
    Average(Vec<Holder<ContextFloatProvider>>),
    Ceiling(Holder<ContextFloatProvider>),
    ConditionalValue { condition: Holder<LootItemCondition>, on_true: Holder<ContextFloatProvider>, on_false: Holder<ContextFloatProvider> },
    ConstantValue(f32),
    Cosine(Holder<ContextFloatProvider>),
    Difference { left: Holder<ContextFloatProvider>, right: Holder<ContextFloatProvider> },
    EnchantmentLevelProvider(LevelBasedValue),
    EnvironmentAttributeValue(&'static EnvironmentAttributeInfo),
    Floor(Holder<ContextFloatProvider>),
    FromInt(Holder<ContextIntProvider>),
    Length(Vec<Holder<ContextFloatProvider>>),
    Maximum(Vec<Holder<ContextFloatProvider>>),
    Minimum(Vec<Holder<ContextFloatProvider>>),
    Modulus { left: Holder<ContextFloatProvider>, right: Holder<ContextFloatProvider> },
    Negate(Holder<ContextFloatProvider>),
    NumberDispatcher { cases: Vec<Case<ContextFloatProvider>>, default_value: Holder<ContextFloatProvider> },
    Power { base: Holder<ContextFloatProvider>, exponent: Holder<ContextFloatProvider> },
    Product(Vec<Holder<ContextFloatProvider>>),
    Quotient { left: Holder<ContextFloatProvider>, right: Holder<ContextFloatProvider> },
    Round(Holder<ContextFloatProvider>),
    Sine(Holder<ContextFloatProvider>),
    SquareRoot(Holder<ContextFloatProvider>),
    StorageValue { access: StoredNumberAccess, fallback: Holder<ContextFloatProvider> },
    Sum(Vec<Holder<ContextFloatProvider>>),
    Truncate(Holder<ContextFloatProvider>),
    UniformGenerator { min: Holder<ContextFloatProvider>, max: Holder<ContextFloatProvider> },
    WeightedListValue(Vec<(Holder<ContextFloatProvider>, i32)>),
}

impl RegistryElement for ContextFloatProvider {
    const REGISTRY: &'static str = "minecraft:context_float_provider";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static ContextFloatProvider> {
        reloadable_server_registries::get().context_float_providers.get(key)
    }
}

pub fn exactly(value: f32) -> Holder<ContextFloatProvider> {
    Holder::direct(ContextFloatProvider::ConstantValue(value))
}

impl ContextFloatProvider {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<ContextFloatProvider>> {
        Holder::decode(tag, ContextFloatProvider::decode)
    }

    fn decode_holders(tag: &Nbt) -> DataResult<Vec<Holder<ContextFloatProvider>>> {
        decode_holder_list(tag, ContextFloatProvider::decode_holder)
    }

    pub fn decode(tag: &Nbt) -> DataResult<ContextFloatProvider> {
        match codec::either(tag, codec::float, ContextFloatProvider::decode_typed)? {
            Ok(value) => Ok(ContextFloatProvider::ConstantValue(value)),
            Err(provider) => Ok(provider),
        }
    }

    fn decode_typed(tag: &Nbt) -> DataResult<ContextFloatProvider> {
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        let holder = |key| codec::decode_field(tag, key, ContextFloatProvider::decode_holder);
        let inputs = || codec::decode_field(tag, "inputs", ContextFloatProvider::decode_holders);
        Ok(match ty.as_str() {
            "minecraft:abs" => ContextFloatProvider::Absolute(holder("input")?),
            "minecraft:avg" => ContextFloatProvider::Average(inputs()?),
            "minecraft:ceil" => ContextFloatProvider::Ceiling(holder("input")?),
            "minecraft:conditional" => ContextFloatProvider::ConditionalValue {
                condition: codec::decode_field(tag, "condition", LootItemCondition::decode_holder)?,
                on_true: holder("on_true")?,
                on_false: codec::decode_or(tag, "on_false", exactly(0.0), ContextFloatProvider::decode_holder)?,
            },
            "minecraft:constant" => ContextFloatProvider::ConstantValue(codec::decode_field(tag, "value", codec::float)?),
            "minecraft:cos" => ContextFloatProvider::Cosine(holder("input")?),
            "minecraft:sub" => ContextFloatProvider::Difference { left: holder("left")?, right: holder("right")? },
            "minecraft:enchantment_level" => ContextFloatProvider::EnchantmentLevelProvider(codec::decode_field(tag, "amount", LevelBasedValue::decode)?),
            "minecraft:environment_attribute" => {
                let name = codec::decode_field(tag, "attribute", codec::identifier)?;
                let attribute = EnvironmentAttributeInfo::by_name(&name).ok_or_else(|| format!("Unknown registry key: {name}"))?;
                if attribute.ty.to_float(attribute.default_value()).is_none() {
                    return Err(format!("{name} cannot be converted to a float"));
                }
                ContextFloatProvider::EnvironmentAttributeValue(attribute)
            }
            "minecraft:floor" => ContextFloatProvider::Floor(holder("input")?),
            "minecraft:from_int" => ContextFloatProvider::FromInt(codec::decode_field(tag, "input", ContextIntProvider::decode_holder)?),
            "minecraft:length" => ContextFloatProvider::Length(inputs()?),
            "minecraft:max" => ContextFloatProvider::Maximum(inputs()?),
            "minecraft:min" => ContextFloatProvider::Minimum(inputs()?),
            "minecraft:mod" => ContextFloatProvider::Modulus { left: holder("left")?, right: holder("right")? },
            "minecraft:negate" => ContextFloatProvider::Negate(holder("input")?),
            "minecraft:number_dispatcher" => ContextFloatProvider::NumberDispatcher {
                cases: codec::decode_field(tag, "cases", |t| codec::list_of(t, |c| Case::decode(c, ContextFloatProvider::decode_holder)))?,
                default_value: codec::decode_or(tag, "default", exactly(0.0), ContextFloatProvider::decode_holder)?,
            },
            "minecraft:pow" => ContextFloatProvider::Power { base: holder("base")?, exponent: holder("exponent")? },
            "minecraft:mul" => ContextFloatProvider::Product(inputs()?),
            "minecraft:div" => ContextFloatProvider::Quotient { left: holder("left")?, right: holder("right")? },
            "minecraft:round" => ContextFloatProvider::Round(holder("input")?),
            "minecraft:sin" => ContextFloatProvider::Sine(holder("input")?),
            "minecraft:sqrt" => ContextFloatProvider::SquareRoot(holder("input")?),
            "minecraft:storage" => ContextFloatProvider::StorageValue {
                access: StoredNumberAccess::decode(tag)?,
                fallback: codec::decode_or(tag, "fallback", exactly(0.0), ContextFloatProvider::decode_holder)?,
            },
            "minecraft:add" => ContextFloatProvider::Sum(inputs()?),
            "minecraft:truncate" => ContextFloatProvider::Truncate(holder("input")?),
            "minecraft:uniform" => ContextFloatProvider::UniformGenerator { min: holder("min")?, max: holder("max")? },
            "minecraft:weighted_list" => ContextFloatProvider::WeightedListValue(codec::decode_field(tag, "distribution", |t| {
                decode_weighted_list(t, ContextFloatProvider::decode_holder)
            })?),
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:context_float_provider_type]: {other}")),
        })
    }

    pub fn get_float(&self, context: &mut LootContext) -> f32 {
        match self.get_float_unsafe(context) {
            Ok(result) if result.is_finite() => result,
            _ => 0.0,
        }
    }

    pub fn get_float_or_throw(&self, context: &mut LootContext) -> ArithmeticResult<f32> {
        let value = self.get_float_unsafe(context)?;
        if value.is_finite() { Ok(value) } else { Err(ArithmeticException::new(format!("Invalid value: {}", crate::nbt::java_float_to_string(value)))) }
    }

    pub fn get_float_unsafe(&self, context: &mut LootContext) -> ArithmeticResult<f32> {
        Ok(match self {
            ContextFloatProvider::Absolute(input) => input.value().get_float_unsafe(context)?.abs(),
            ContextFloatProvider::Average(inputs) => {
                let (mut sum, mut count) = (0.0f32, 0);
                for input in inputs {
                    sum += input.value().get_float_unsafe(context)?;
                    count += 1;
                }
                sum / count as f32
            }
            ContextFloatProvider::Ceiling(input) => mth::ceil(input.value().get_float_unsafe(context)? as f64) as f32,
            ContextFloatProvider::ConditionalValue { condition, on_true, on_false } => {
                let value = if condition.value().test(context) { on_true } else { on_false };
                value.value().get_float_unsafe(context)?
            }
            ContextFloatProvider::ConstantValue(value) => *value,
            ContextFloatProvider::Cosine(input) => mth::cos(input.value().get_float_unsafe(context)? as f64),
            ContextFloatProvider::Difference { left, right } => left.value().get_float_unsafe(context)? - right.value().get_float_unsafe(context)?,
            ContextFloatProvider::EnchantmentLevelProvider(amount) => amount.calculate(context.params.enchantment_level.unwrap_or(0)),
            ContextFloatProvider::EnvironmentAttributeValue(attribute) => {
                let value = context.environment_attribute_value(attribute);
                attribute.ty.to_float(&value).unwrap_or(0.0)
            }
            ContextFloatProvider::Floor(input) => input.value().get_float_unsafe(context)?.floor(),
            ContextFloatProvider::FromInt(input) => input.value().get_int_unsafe(context)? as f32,
            ContextFloatProvider::Length(inputs) => {
                let mut sum_of_squares = 0.0f32;
                for input in inputs {
                    let value = input.value().get_float_unsafe(context)?;
                    sum_of_squares += value * value;
                }
                mth::sqrt(sum_of_squares)
            }
            ContextFloatProvider::Maximum(inputs) => {
                let mut value = -f32::MAX;
                for input in inputs {
                    value = java_max_f(value, input.value().get_float_unsafe(context)?);
                }
                value
            }
            ContextFloatProvider::Minimum(inputs) => {
                let mut value = f32::MAX;
                for input in inputs {
                    value = java_min_f(value, input.value().get_float_unsafe(context)?);
                }
                value
            }
            ContextFloatProvider::Modulus { left, right } => {
                let right_value = right.value().get_float_unsafe(context)?;
                if right_value == 0.0 { f32::NAN } else { left.value().get_float_unsafe(context)? % right_value }
            }
            ContextFloatProvider::Negate(input) => -input.value().get_float_unsafe(context)?,
            ContextFloatProvider::NumberDispatcher { cases, default_value } => {
                let mut selected = default_value;
                for case in cases {
                    if case.test(context) {
                        selected = &case.value;
                        break;
                    }
                }
                selected.value().get_float_unsafe(context)?
            }
            ContextFloatProvider::Power { base, exponent } => {
                let (base, exponent) = (base.value().get_float_unsafe(context)?, exponent.value().get_float_unsafe(context)?);
                if base == 0.0 && exponent == 0.0 { f32::NAN } else { (base as f64).powf(exponent as f64) as f32 }
            }
            ContextFloatProvider::Product(inputs) => {
                let mut value = 1.0f32;
                for input in inputs {
                    value *= input.value().get_float_unsafe(context)?;
                }
                value
            }
            ContextFloatProvider::Quotient { left, right } => left.value().get_float_unsafe(context)? / right.value().get_float_unsafe(context)?,
            ContextFloatProvider::Round(input) => crate::util::java_round_f(input.value().get_float_unsafe(context)?) as f32,
            ContextFloatProvider::Sine(input) => mth::sin(input.value().get_float_unsafe(context)? as f64),
            ContextFloatProvider::SquareRoot(input) => mth::sqrt(input.value().get_float_unsafe(context)?),
            ContextFloatProvider::StorageValue { access, fallback } => match access.get_numeric_tag(context) {
                Some(value) => crate::world::item::component::codec::as_f32(&value),
                None => fallback.value().get_float_unsafe(context)?,
            },
            ContextFloatProvider::Sum(inputs) => {
                let mut value = 0.0f32;
                for input in inputs {
                    value += input.value().get_float_unsafe(context)?;
                }
                value
            }
            ContextFloatProvider::Truncate(input) => {
                let value = input.value().get_float_unsafe(context)?;
                if value > 0.0 { value.floor() } else { value.ceil() }
            }
            ContextFloatProvider::UniformGenerator { min, max } => {
                let (min, max) = (min.value().get_float_unsafe(context)?, max.value().get_float_unsafe(context)?);
                mth::next_float(&mut context.random(),min, max)
            }
            ContextFloatProvider::WeightedListValue(distribution) => get_random(distribution, context).value().get_float_unsafe(context)?,
        })
    }
}
