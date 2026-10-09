use std::sync::Arc;

use serde_json::Value;

use crate::world::level::levelgen::density_function::float;

#[derive(Clone, PartialEq, Debug)]
pub struct AmbientParticle {
    pub particle: Arc<Value>,
    pub probability: f32,
}

impl AmbientParticle {
    pub fn parse(json: &Value) -> Result<AmbientParticle, String> {
        let particle = json.get("particle").ok_or("missing particle")?;
        let probability = float(json.get("probability").ok_or("missing probability")?)?;
        if !(0.0..=1.0).contains(&probability) {
            return Err(format!("Value {probability} outside of range [0.0:1.0]"));
        }
        Ok(AmbientParticle { particle: Arc::new(particle.clone()), probability })
    }
}
