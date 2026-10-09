use crisp_core::RandomSource;
use serde_json::Value;

use super::random::WeightedList;

#[derive(Clone, Debug)]
pub enum IntProvider {
    Constant(i32),
    Uniform { min_inclusive: i32, max_inclusive: i32 },
    BiasedToBottom { min_inclusive: i32, max_inclusive: i32 },
    VeryBiasedToBottom { min_inclusive: i32, max_inclusive: i32 },
    Clamped { source: Box<IntProvider>, min_inclusive: i32, max_inclusive: i32 },
    WeightedList { distribution: WeightedList<IntProvider>, min_value: i32, max_value: i32 },
    ClampedNormal { mean: f32, deviation: f32, min_inclusive: i32, max_inclusive: i32 },
    Trapezoid { min_inclusive: i32, max_inclusive: i32, plateau: i32 },
}

pub fn random_between_inclusive<R: RandomSource + ?Sized>(random: &mut R, min: i32, max_inclusive: i32) -> i32 {
    random.next_int_bounded(max_inclusive - min + 1) + min
}

pub fn random_between<R: RandomSource + ?Sized>(random: &mut R, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

pub fn normal<R: RandomSource + ?Sized>(random: &mut R, mean: f32, deviation: f32) -> f32 {
    mean + random.next_gaussian() as f32 * deviation
}

fn int_field(json: &Value, key: &str) -> Result<i32, String> {
    json.get(key).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| format!("missing {key}"))
}

fn float_field(json: &Value, key: &str) -> Result<f32, String> {
    json.get(key).ok_or_else(|| format!("missing {key}")).and_then(float)
}

pub fn float(v: &Value) -> Result<f32, String> {
    let d = v.as_f64().ok_or_else(|| format!("expected a number, got {v}"))?;
    format!("{d}").parse::<f32>().map_err(|e| e.to_string())
}

impl IntProvider {
    pub fn parse(json: &Value) -> Result<IntProvider, String> {
        if let Some(value) = json.as_i64() {
            return Ok(IntProvider::Constant(value as i32));
        }
        let ty = json.get("type").and_then(Value::as_str).ok_or("int provider without type")?;
        let min = || int_field(json, "min_inclusive");
        let max = || int_field(json, "max_inclusive");
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "constant" => IntProvider::Constant(int_field(json, "value")?),
            "uniform" => IntProvider::Uniform { min_inclusive: min()?, max_inclusive: max()? },
            "biased_to_bottom" => IntProvider::BiasedToBottom { min_inclusive: min()?, max_inclusive: max()? },
            "very_biased_to_bottom" => IntProvider::VeryBiasedToBottom { min_inclusive: min()?, max_inclusive: max()? },
            "clamped" => IntProvider::Clamped {
                source: Box::new(IntProvider::parse(json.get("source").ok_or("missing source")?)?),
                min_inclusive: min()?,
                max_inclusive: max()?,
            },
            "weighted_list" => {
                let distribution = WeightedList::parse(json.get("distribution").ok_or("missing distribution")?, IntProvider::parse)?;
                let min_value = distribution.items.iter().map(|(p, _)| p.min_inclusive()).min().ok_or("empty distribution")?;
                let max_value = distribution.items.iter().map(|(p, _)| p.max_inclusive()).max().ok_or("empty distribution")?;
                IntProvider::WeightedList { distribution, min_value, max_value }
            }
            "clamped_normal" => IntProvider::ClampedNormal {
                mean: float_field(json, "mean")?,
                deviation: float_field(json, "deviation")?,
                min_inclusive: min()?,
                max_inclusive: max()?,
            },
            "trapezoid" => {
                IntProvider::Trapezoid { min_inclusive: int_field(json, "min")?, max_inclusive: int_field(json, "max")?, plateau: int_field(json, "plateau")? }
            }
            other => return Err(format!("unknown int provider {other}")),
        })
    }

    pub fn sample<R: RandomSource + ?Sized>(&self, random: &mut R) -> i32 {
        match self {
            IntProvider::Constant(value) => *value,
            IntProvider::Uniform { min_inclusive, max_inclusive } => random_between_inclusive(random, *min_inclusive, *max_inclusive),
            IntProvider::BiasedToBottom { min_inclusive, max_inclusive } => {
                let bound = random.next_int_bounded(max_inclusive - min_inclusive + 1) + 1;
                min_inclusive + random.next_int_bounded(bound)
            }
            IntProvider::VeryBiasedToBottom { min_inclusive, max_inclusive } => {
                let a = random.next_int_bounded(max_inclusive - min_inclusive + 1) + 1;
                let b = random.next_int_bounded(a) + 1;
                min_inclusive + random.next_int_bounded(b)
            }
            IntProvider::Clamped { source, min_inclusive, max_inclusive } => source.sample(random).clamp(*min_inclusive, *max_inclusive),
            IntProvider::WeightedList { distribution, .. } => distribution.get_random_or_throw(random).sample(random),
            IntProvider::ClampedNormal { mean, deviation, min_inclusive, max_inclusive } => {
                normal(random, *mean, *deviation).clamp(*min_inclusive as f32, *max_inclusive as f32) as i32
            }
            IntProvider::Trapezoid { min_inclusive, max_inclusive, plateau } => {
                let (min, max, plateau) = (*min_inclusive, *max_inclusive, *plateau);
                if plateau == 0 && max == -min {
                    return random.next_int_bounded(max + 1) - random.next_int_bounded(max + 1);
                }
                let range = max - min;
                if plateau == range {
                    return random_between_inclusive(random, min, max);
                }
                let plateau_start = (range - plateau) / 2;
                let plateau_end = range - plateau_start;
                min + random_between_inclusive(random, 0, plateau_end) + random_between_inclusive(random, 0, plateau_start)
            }
        }
    }

    pub fn min_inclusive(&self) -> i32 {
        match self {
            IntProvider::Constant(value) => *value,
            IntProvider::Uniform { min_inclusive, .. }
            | IntProvider::BiasedToBottom { min_inclusive, .. }
            | IntProvider::VeryBiasedToBottom { min_inclusive, .. }
            | IntProvider::Trapezoid { min_inclusive, .. } => *min_inclusive,
            IntProvider::Clamped { source, min_inclusive, .. } => source.min_inclusive().max(*min_inclusive),
            IntProvider::WeightedList { min_value, .. } => *min_value,
            IntProvider::ClampedNormal { min_inclusive, .. } => *min_inclusive,
        }
    }

    pub fn max_inclusive(&self) -> i32 {
        match self {
            IntProvider::Constant(value) => *value,
            IntProvider::Uniform { max_inclusive, .. }
            | IntProvider::BiasedToBottom { max_inclusive, .. }
            | IntProvider::VeryBiasedToBottom { max_inclusive, .. }
            | IntProvider::Trapezoid { max_inclusive, .. } => *max_inclusive,
            IntProvider::Clamped { source, max_inclusive, .. } => source.max_inclusive().min(*max_inclusive),
            IntProvider::WeightedList { max_value, .. } => *max_value,
            IntProvider::ClampedNormal { max_inclusive, .. } => *max_inclusive,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FloatProvider {
    Constant(f32),
    Uniform { min_inclusive: f32, max_exclusive: f32 },
    ClampedNormal { mean: f32, deviation: f32, min: f32, max: f32 },
    Trapezoid { min: f32, max: f32, plateau: f32 },
}

impl FloatProvider {
    pub fn parse(json: &Value) -> Result<FloatProvider, String> {
        if json.is_number() {
            return Ok(FloatProvider::Constant(float(json)?));
        }
        let ty = json.get("type").and_then(Value::as_str).ok_or("float provider without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "constant" => FloatProvider::Constant(float_field(json, "value")?),
            "uniform" => FloatProvider::Uniform { min_inclusive: float_field(json, "min_inclusive")?, max_exclusive: float_field(json, "max_exclusive")? },
            "clamped_normal" => FloatProvider::ClampedNormal {
                mean: float_field(json, "mean")?,
                deviation: float_field(json, "deviation")?,
                min: float_field(json, "min")?,
                max: float_field(json, "max")?,
            },
            "trapezoid" => FloatProvider::Trapezoid { min: float_field(json, "min")?, max: float_field(json, "max")?, plateau: float_field(json, "plateau")? },
            other => return Err(format!("unknown float provider {other}")),
        })
    }

    pub fn sample<R: RandomSource + ?Sized>(&self, random: &mut R) -> f32 {
        match *self {
            FloatProvider::Constant(value) => value,
            FloatProvider::Uniform { min_inclusive, max_exclusive } => random_between(random, min_inclusive, max_exclusive),
            FloatProvider::ClampedNormal { mean, deviation, min, max } => normal(random, mean, deviation).clamp(min, max),
            FloatProvider::Trapezoid { min, max, plateau } => {
                let range = max - min;
                let plateau_start = (range - plateau) / 2.0;
                let plateau_end = range - plateau_start;
                min + random.next_float() * plateau_end + random.next_float() * plateau_start
            }
        }
    }
}
