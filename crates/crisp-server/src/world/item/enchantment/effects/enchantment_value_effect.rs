use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::enchantment::level_based_value::LevelBasedValue;

#[derive(Clone, Debug, PartialEq)]
pub enum EnchantmentValueEffect {
    AddValue(LevelBasedValue),
    AllOf(Vec<EnchantmentValueEffect>),
    MultiplyValue(LevelBasedValue),
    RemoveBinomial(LevelBasedValue),
    ScaleExponentially { base: LevelBasedValue, exponent: LevelBasedValue },
    SetValue(LevelBasedValue),
}

impl EnchantmentValueEffect {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentValueEffect> {
        codec::map(tag)?;
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        let value = |key: &str| codec::decode_field(tag, key, LevelBasedValue::decode);
        Ok(match ty.as_str() {
            "minecraft:add" => EnchantmentValueEffect::AddValue(value("value")?),
            "minecraft:all_of" => EnchantmentValueEffect::AllOf(codec::decode_field(tag, "effects", |t| codec::list_of(t, EnchantmentValueEffect::decode))?),
            "minecraft:multiply" => EnchantmentValueEffect::MultiplyValue(value("factor")?),
            "minecraft:remove_binomial" => EnchantmentValueEffect::RemoveBinomial(value("chance")?),
            "minecraft:exponential" => EnchantmentValueEffect::ScaleExponentially { base: value("base")?, exponent: value("exponent")? },
            "minecraft:set" => EnchantmentValueEffect::SetValue(value("value")?),
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_value_effect_type]: {other}")),
        })
    }

    pub fn process(&self, enchantment_level: i32, random: &mut impl RandomSource, input_value: f32) -> f32 {
        match self {
            EnchantmentValueEffect::AddValue(value) => input_value + value.calculate(enchantment_level),
            EnchantmentValueEffect::AllOf(effects) => effects.iter().fold(input_value, |value, effect| effect.process(enchantment_level, random, value)),
            EnchantmentValueEffect::MultiplyValue(factor) => input_value * factor.calculate(enchantment_level),
            EnchantmentValueEffect::RemoveBinomial(chance) => {
                let (n, p) = (input_value, chance.calculate(enchantment_level));
                let mut drop = 0;
                if !(n <= 128.0) && !(n * p < 20.0) && !(n * (1.0 - p) < 20.0) {
                    let miu = ((n * p) as f64).floor();
                    let sigma = ((n * p * (1.0 - p)) as f64).sqrt();
                    drop = (miu + random.next_gaussian() * sigma + 0.5).floor() as i64 as i32;
                    drop = drop.clamp(0, n as i32);
                } else {
                    let mut y = 0;
                    while (y as f32) < n {
                        if random.next_float() < p {
                            drop += 1;
                        }
                        y += 1;
                    }
                }
                n - drop as f32
            }
            EnchantmentValueEffect::ScaleExponentially { base, exponent } => {
                (input_value as f64 * (base.calculate(enchantment_level) as f64).powf(exponent.calculate(enchantment_level) as f64)) as f32
            }
            EnchantmentValueEffect::SetValue(value) => value.calculate(enchantment_level),
        }
    }
}
