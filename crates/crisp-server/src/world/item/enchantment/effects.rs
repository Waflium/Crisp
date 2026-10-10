use crisp_core::random::RandomSource;
use serde_json::Value;

use super::level_based_value::LevelBasedValue;

#[derive(Clone, Debug, PartialEq)]
pub enum EnchantmentValueEffect {
    Add(LevelBasedValue),
    AllOf(Vec<EnchantmentValueEffect>),
    Multiply(LevelBasedValue),
    RemoveBinomial(LevelBasedValue),
    Exponential { base: LevelBasedValue, exponent: LevelBasedValue },
    Set(LevelBasedValue),
}

impl EnchantmentValueEffect {
    pub fn parse(v: &Value) -> Option<EnchantmentValueEffect> {
        let value = |key: &str| LevelBasedValue::parse(&v[key]);
        Some(match v["type"].as_str()?.trim_start_matches("minecraft:") {
            "add" => EnchantmentValueEffect::Add(value("value")?),
            "all_of" => EnchantmentValueEffect::AllOf(v["effects"].as_array()?.iter().map(EnchantmentValueEffect::parse).collect::<Option<_>>()?),
            "multiply" => EnchantmentValueEffect::Multiply(value("factor")?),
            "remove_binomial" => EnchantmentValueEffect::RemoveBinomial(value("chance")?),
            "exponential" => EnchantmentValueEffect::Exponential { base: value("base")?, exponent: value("exponent")? },
            "set" => EnchantmentValueEffect::Set(value("value")?),
            _ => return None,
        })
    }

    pub fn process(&self, level: i32, random: &mut impl RandomSource, value: f32) -> f32 {
        match self {
            EnchantmentValueEffect::Add(add) => value + add.calculate(level),
            EnchantmentValueEffect::AllOf(effects) => effects.iter().fold(value, |value, effect| effect.process(level, random, value)),
            EnchantmentValueEffect::Multiply(factor) => value * factor.calculate(level),
            EnchantmentValueEffect::RemoveBinomial(chance) => {
                let (n, p) = (value, chance.calculate(level));
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
            EnchantmentValueEffect::Exponential { base, exponent } => (value as f64 * (base.calculate(level) as f64).powf(exponent.calculate(level) as f64)) as f32,
            EnchantmentValueEffect::Set(set) => set.calculate(level),
        }
    }
}
