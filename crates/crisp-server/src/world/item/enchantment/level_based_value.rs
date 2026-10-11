use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum LevelBasedValue {
    Constant(f32),
    Clamped { value: Box<LevelBasedValue>, min: f32, max: f32 },
    Exponent { base: Box<LevelBasedValue>, power: Box<LevelBasedValue> },
    Fraction { numerator: Box<LevelBasedValue>, denominator: Box<LevelBasedValue> },
    LevelsSquared { added: f32 },
    Linear { base: f32, per_level_above_first: f32 },
    Lookup { values: Vec<f32>, fallback: Box<LevelBasedValue> },
}

fn float(v: &Value) -> Option<f32> {
    v.as_f64().map(|v| v as f32)
}

impl LevelBasedValue {
    pub fn parse(v: &Value) -> Option<LevelBasedValue> {
        if let Some(constant) = float(v) {
            return Some(LevelBasedValue::Constant(constant));
        }
        let boxed = |key: &str| LevelBasedValue::parse(&v[key]).map(Box::new);
        Some(match v["type"].as_str()?.trim_start_matches("minecraft:") {
            "clamped" => LevelBasedValue::Clamped { value: boxed("value")?, min: float(&v["min"])?, max: float(&v["max"])? },
            "exponent" => LevelBasedValue::Exponent { base: boxed("base")?, power: boxed("power")? },
            "fraction" => LevelBasedValue::Fraction { numerator: boxed("numerator")?, denominator: boxed("denominator")? },
            "levels_squared" => LevelBasedValue::LevelsSquared { added: float(&v["added"])? },
            "linear" => LevelBasedValue::Linear { base: float(&v["base"])?, per_level_above_first: float(&v["per_level_above_first"])? },
            "lookup" => LevelBasedValue::Lookup { values: v["values"].as_array()?.iter().map(float).collect::<Option<_>>()?, fallback: boxed("fallback")? },
            _ => return None,
        })
    }

    pub fn decode(tag: &crisp_protocol::nbt::Nbt) -> crate::util::codec::DataResult<LevelBasedValue> {
        use crate::util::codec;
        if let Ok(constant) = codec::float(tag) {
            return Ok(LevelBasedValue::Constant(constant));
        }
        let boxed = |key| codec::decode_field(tag, key, LevelBasedValue::decode).map(Box::new);
        let float = |key| codec::decode_field(tag, key, codec::float);
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        Ok(match ty.as_str() {
            "minecraft:clamped" => LevelBasedValue::Clamped { value: boxed("value")?, min: float("min")?, max: float("max")? },
            "minecraft:exponent" => LevelBasedValue::Exponent { base: boxed("base")?, power: boxed("power")? },
            "minecraft:fraction" => LevelBasedValue::Fraction { numerator: boxed("numerator")?, denominator: boxed("denominator")? },
            "minecraft:levels_squared" => LevelBasedValue::LevelsSquared { added: float("added")? },
            "minecraft:linear" => LevelBasedValue::Linear { base: float("base")?, per_level_above_first: float("per_level_above_first")? },
            "minecraft:lookup" => LevelBasedValue::Lookup { values: codec::decode_field(tag, "values", |t| codec::list_of(t, codec::float))?, fallback: boxed("fallback")? },
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_level_based_value_type]: {other}")),
        })
    }

    pub fn calculate(&self, level: i32) -> f32 {
        match self {
            LevelBasedValue::Constant(value) => *value,
            LevelBasedValue::Clamped { value, min, max } => crisp_core::mth::clamp_f(value.calculate(level), *min, *max),
            LevelBasedValue::Exponent { base, power } => (base.calculate(level) as f64).powf(power.calculate(level) as f64) as f32,
            LevelBasedValue::Fraction { numerator, denominator } => {
                let denominator = denominator.calculate(level);
                if denominator == 0.0 { 0.0 } else { numerator.calculate(level) / denominator }
            }
            LevelBasedValue::LevelsSquared { added } => level.wrapping_mul(level) as f32 + added,
            LevelBasedValue::Linear { base, per_level_above_first } => base + per_level_above_first * (level - 1) as f32,
            LevelBasedValue::Lookup { values, fallback } => values.get((level - 1) as usize).copied().unwrap_or_else(|| fallback.calculate(level)),
        }
    }
}
