use serde_json::Value as Json;

use super::FloatWithAlpha;
use crate::util::{java_max_f, java_min_f};
use crate::world::attribute::{LerpFunction, Value};
use crate::world::level::levelgen::density_function::float;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FloatModifier {
    AlphaBlend,
    Add,
    Subtract,
    Multiply,
    Minimum,
    Maximum,
}

impl FloatModifier {
    pub fn apply(self, subject: f32, argument: &Value) -> Value {
        if self == FloatModifier::AlphaBlend {
            let Value::FloatWithAlpha(argument) = argument else { panic!("{argument:?} is not FloatWithAlpha") };
            return Value::Float(crisp_core::mth::lerp_f(argument.alpha, subject, argument.value));
        }
        let argument = argument.as_float();
        Value::Float(match self {
            FloatModifier::AlphaBlend => unreachable!(),
            FloatModifier::Add => subject + argument,
            FloatModifier::Subtract => subject - argument,
            FloatModifier::Multiply => subject * argument,
            FloatModifier::Minimum => java_min_f(subject, argument),
            FloatModifier::Maximum => java_max_f(subject, argument),
        })
    }

    pub fn parse_argument(self, json: &Json) -> Result<Value, String> {
        match self {
            FloatModifier::AlphaBlend => FloatWithAlpha::parse(json).map(Value::FloatWithAlpha),
            _ => float(json).map(Value::Float),
        }
    }

    pub fn argument_keyframe_lerp(self) -> LerpFunction {
        match self {
            FloatModifier::AlphaBlend => LerpFunction::FloatWithAlpha,
            _ => LerpFunction::Float,
        }
    }
}
