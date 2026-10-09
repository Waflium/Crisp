pub mod ambient_particle;
pub mod attribute_range;
pub mod attribute_type;
pub mod bed_rule;
pub mod environment_attribute;
pub mod environment_attribute_map;
pub mod environment_attribute_system;
pub mod environment_attributes;
pub mod lerp_function;
pub mod modifier;
#[cfg(test)]
mod tests;
pub mod weather_attributes;

use std::sync::Arc;

pub use ambient_particle::AmbientParticle;
pub use attribute_range::AttributeRange;
pub use attribute_type::AttributeType;
pub use bed_rule::BedRule;
pub use environment_attribute::EnvironmentAttribute;
pub use environment_attribute_map::EnvironmentAttributeMap;
pub use environment_attribute_system::EnvironmentAttributeSystem;
pub use lerp_function::LerpFunction;

use crate::util::argb::{Vec3f, Vec4f};
use crate::util::tri_state::TriState;
use crate::world::entity::schedule::Activity;
use crate::world::level::biome::mob_spawn_settings::MobSpawnSettings;
use crate::world::level::moon_phase::MoonPhase;
use modifier::{BlendToGray, FloatWithAlpha};

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Boolean(bool),
    TriState(TriState),
    Float(f32),
    Integer(i32),
    Rgb(Vec3f),
    Argb(Vec4f),
    MoonPhase(MoonPhase),
    Activity(Activity),
    BedRule(Arc<BedRule>),
    AmbientParticles(Arc<Vec<AmbientParticle>>),
    MobSpawnSettings(Arc<MobSpawnSettings>),
    Data(Arc<serde_json::Value>),
    FloatWithAlpha(FloatWithAlpha),
    BlendToGray(BlendToGray),
}

impl Value {
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            _ => panic!("{self:?} is not a boolean"),
        }
    }

    pub fn as_float(&self) -> f32 {
        match self {
            Value::Float(f) => *f,
            _ => panic!("{self:?} is not a float"),
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Value::Integer(i) => *i,
            _ => panic!("{self:?} is not an integer"),
        }
    }

    pub fn as_rgb(&self) -> Vec3f {
        match self {
            Value::Rgb(c) => *c,
            _ => panic!("{self:?} is not an RGB colour"),
        }
    }

    pub fn as_argb(&self) -> Vec4f {
        match self {
            Value::Argb(c) => *c,
            _ => panic!("{self:?} is not an ARGB colour"),
        }
    }
}

pub trait AttributeValue: Sized {
    fn from_value(value: &Value) -> Self;
}

macro_rules! attribute_value {
    ($t:ty, $($variant:ident)|+) => {
        impl AttributeValue for $t {
            fn from_value(value: &Value) -> Self {
                match value {
                    $(Value::$variant(v) => v.clone(),)+
                    _ => panic!(concat!("{:?} is not a ", stringify!($t)), value),
                }
            }
        }
    };
}

attribute_value!(bool, Boolean);
attribute_value!(TriState, TriState);
attribute_value!(f32, Float);
attribute_value!(i32, Integer);
attribute_value!(Vec3f, Rgb);
attribute_value!(Vec4f, Argb);
attribute_value!(MoonPhase, MoonPhase);
attribute_value!(Activity, Activity);
attribute_value!(Arc<BedRule>, BedRule);
attribute_value!(Arc<Vec<AmbientParticle>>, AmbientParticles);
attribute_value!(Arc<MobSpawnSettings>, MobSpawnSettings);
attribute_value!(Arc<serde_json::Value>, Data);

fn parse_bool(json: &serde_json::Value) -> Result<bool, String> {
    match json {
        serde_json::Value::Bool(b) => Ok(*b),
        serde_json::Value::Number(n) => Ok(n.as_f64().is_some_and(|v| v as i64 as i8 != 0)),
        _ => Err(format!("not a boolean: {json}")),
    }
}

fn parse_int(json: &serde_json::Value) -> Result<i32, String> {
    json.as_i64().map(|i| i as i32).or_else(|| json.as_f64().map(|d| d as i32)).ok_or_else(|| format!("not a number: {json}"))
}
