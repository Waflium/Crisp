use serde_json::Value as Json;

use crate::world::level::levelgen::density_function::float;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FloatWithAlpha {
    pub value: f32,
    pub alpha: f32,
}

impl FloatWithAlpha {
    pub fn parse(json: &Json) -> Result<FloatWithAlpha, String> {
        if json.is_number() {
            return Ok(FloatWithAlpha { value: float(json)?, alpha: 1.0 });
        }
        let value = float(json.get("value").ok_or("missing value")?)?;
        let alpha = json.get("alpha").map_or(Ok(1.0), float)?;
        if !(0.0..=1.0).contains(&alpha) {
            return Err(format!("Value {alpha} outside of range [0.0:1.0]"));
        }
        Ok(FloatWithAlpha { value, alpha })
    }
}
