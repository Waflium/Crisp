use crisp_core::RandomSource;
use serde_json::Value;

use super::material::{VerticalAnchor, WorldGenerationContext};
use crate::util::random::WeightedList;
use crate::util::valueproviders::random_between_inclusive;

#[derive(Clone, Debug)]
pub enum HeightProvider {
    Constant(VerticalAnchor),
    Uniform { min_inclusive: VerticalAnchor, max_inclusive: VerticalAnchor },
    BiasedToBottom { min_inclusive: VerticalAnchor, max_inclusive: VerticalAnchor, inner: i32 },
    VeryBiasedToBottom { min_inclusive: VerticalAnchor, max_inclusive: VerticalAnchor, inner: i32 },
    Trapezoid { min_inclusive: VerticalAnchor, max_inclusive: VerticalAnchor, plateau: i32 },
    WeightedList(WeightedList<HeightProvider>),
}

pub fn next_int<R: RandomSource + ?Sized>(random: &mut R, min_inclusive: i32, max_inclusive: i32) -> i32 {
    if min_inclusive >= max_inclusive { min_inclusive } else { random.next_int_bounded(max_inclusive - min_inclusive + 1) + min_inclusive }
}

impl HeightProvider {
    pub fn parse(json: &Value) -> Result<HeightProvider, String> {
        let Some(ty) = json.get("type").and_then(Value::as_str) else {
            return Ok(HeightProvider::Constant(VerticalAnchor::parse(json)?));
        };
        let anchor = |key: &str| VerticalAnchor::parse(json.get(key).ok_or_else(|| format!("missing {key}"))?);
        let int_or = |key: &str, default: i32| json.get(key).and_then(Value::as_i64).map_or(default, |v| v as i32);
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "constant" => HeightProvider::Constant(anchor("value")?),
            "uniform" => HeightProvider::Uniform { min_inclusive: anchor("min_inclusive")?, max_inclusive: anchor("max_inclusive")? },
            "biased_to_bottom" => {
                HeightProvider::BiasedToBottom { min_inclusive: anchor("min_inclusive")?, max_inclusive: anchor("max_inclusive")?, inner: int_or("inner", 1) }
            }
            "very_biased_to_bottom" => {
                HeightProvider::VeryBiasedToBottom { min_inclusive: anchor("min_inclusive")?, max_inclusive: anchor("max_inclusive")?, inner: int_or("inner", 1) }
            }
            "trapezoid" => {
                HeightProvider::Trapezoid { min_inclusive: anchor("min_inclusive")?, max_inclusive: anchor("max_inclusive")?, plateau: int_or("plateau", 0) }
            }
            "weighted_list" => HeightProvider::WeightedList(WeightedList::parse(json.get("distribution").ok_or("missing distribution")?, HeightProvider::parse)?),
            other => return Err(format!("unknown height provider {other}")),
        })
    }

    pub fn sample<R: RandomSource + ?Sized>(&self, random: &mut R, context: &WorldGenerationContext) -> i32 {
        match self {
            HeightProvider::Constant(value) => value.resolve_y(context),
            HeightProvider::Uniform { min_inclusive, max_inclusive } => {
                let (min, max) = (min_inclusive.resolve_y(context), max_inclusive.resolve_y(context));
                if min > max { min } else { random_between_inclusive(random, min, max) }
            }
            HeightProvider::BiasedToBottom { min_inclusive, max_inclusive, inner } => {
                let (min, max) = (min_inclusive.resolve_y(context), max_inclusive.resolve_y(context));
                if max - min - inner + 1 <= 0 {
                    return min;
                }
                let limit = random.next_int_bounded(max - min - inner + 1);
                random.next_int_bounded(limit + inner) + min
            }
            HeightProvider::VeryBiasedToBottom { min_inclusive, max_inclusive, inner } => {
                let (min, max) = (min_inclusive.resolve_y(context), max_inclusive.resolve_y(context));
                if max - min - inner + 1 <= 0 {
                    return min;
                }
                let upper_inclusive = next_int(random, min + inner, max);
                let biased_upper_inclusive = next_int(random, min, upper_inclusive - 1);
                next_int(random, min, biased_upper_inclusive - 1 + inner)
            }
            HeightProvider::Trapezoid { min_inclusive, max_inclusive, plateau } => {
                let (min, max) = (min_inclusive.resolve_y(context), max_inclusive.resolve_y(context));
                if min > max {
                    return min;
                }
                let range = max - min;
                if *plateau >= range {
                    return random_between_inclusive(random, min, max);
                }
                let plateau_start = (range - plateau) / 2;
                let plateau_end = range - plateau_start;
                min + random_between_inclusive(random, 0, plateau_end) + random_between_inclusive(random, 0, plateau_start)
            }
            HeightProvider::WeightedList(distribution) => distribution.get_random_or_throw(random).sample(random, context),
        }
    }
}
