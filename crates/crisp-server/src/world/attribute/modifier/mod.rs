pub mod boolean_modifier;
pub mod color_modifier;
pub mod float_modifier;
pub mod float_with_alpha;
pub mod integer_modifier;
pub mod list_modifier;
pub mod mob_spawn_settings_modifier;

pub use boolean_modifier::BooleanModifier;
pub use color_modifier::{BlendToGray, ColorModifier};
pub use float_modifier::FloatModifier;
pub use float_with_alpha::FloatWithAlpha;
pub use integer_modifier::IntegerModifier;

use serde_json::Value as Json;

use super::environment_attribute::EnvironmentAttributeInfo;
use super::{LerpFunction, Value};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum OperationId {
    Override,
    AlphaBlend,
    Add,
    Subtract,
    Multiply,
    BlendToGray,
    Minimum,
    Maximum,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Append,
    Overlay,
}

impl OperationId {
    pub const VALUES: [OperationId; 16] = [
        OperationId::Override,
        OperationId::AlphaBlend,
        OperationId::Add,
        OperationId::Subtract,
        OperationId::Multiply,
        OperationId::BlendToGray,
        OperationId::Minimum,
        OperationId::Maximum,
        OperationId::And,
        OperationId::Nand,
        OperationId::Or,
        OperationId::Nor,
        OperationId::Xor,
        OperationId::Xnor,
        OperationId::Append,
        OperationId::Overlay,
    ];

    pub fn name(self) -> &'static str {
        [
            "override",
            "alpha_blend",
            "add",
            "subtract",
            "multiply",
            "blend_to_gray",
            "minimum",
            "maximum",
            "and",
            "nand",
            "or",
            "nor",
            "xor",
            "xnor",
            "append",
            "overlay",
        ][self as usize]
    }

    pub fn by_name(name: &str) -> Option<OperationId> {
        Self::VALUES.into_iter().find(|o| o.name() == name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AttributeModifier {
    Override,
    Boolean(BooleanModifier),
    Float(FloatModifier),
    Integer(IntegerModifier),
    Color(ColorModifier),
    Append,
    Overlay,
}

impl AttributeModifier {
    pub fn apply(self, subject: &Value, argument: &Value) -> Value {
        match self {
            AttributeModifier::Override => argument.clone(),
            AttributeModifier::Boolean(m) => Value::Boolean(m.apply(subject.as_bool(), argument.as_bool())),
            AttributeModifier::Float(m) => m.apply(subject.as_float(), argument),
            AttributeModifier::Integer(m) => Value::Integer(m.apply(subject.as_int(), argument.as_int())),
            AttributeModifier::Color(m) => m.apply(subject, argument),
            AttributeModifier::Append => list_modifier::append(subject, argument),
            AttributeModifier::Overlay => mob_spawn_settings_modifier::overlay(subject, argument),
        }
    }

    pub fn parse_argument(self, attribute: &EnvironmentAttributeInfo, json: &Json) -> Result<Value, String> {
        match self {
            AttributeModifier::Override | AttributeModifier::Append | AttributeModifier::Overlay => attribute.parse_value(json),
            AttributeModifier::Boolean(_) => super::parse_bool(json).map(Value::Boolean),
            AttributeModifier::Float(m) => m.parse_argument(json),
            AttributeModifier::Integer(_) => super::parse_int(json).map(Value::Integer),
            AttributeModifier::Color(m) => m.parse_argument(json),
        }
    }

    pub fn argument_keyframe_lerp(self, attribute: &EnvironmentAttributeInfo) -> LerpFunction {
        match self {
            AttributeModifier::Override | AttributeModifier::Append | AttributeModifier::Overlay => attribute.ty.keyframe_lerp(),
            AttributeModifier::Boolean(_) => LerpFunction::CONSTANT,
            AttributeModifier::Float(m) => m.argument_keyframe_lerp(),
            AttributeModifier::Integer(_) => LerpFunction::Integer,
            AttributeModifier::Color(m) => m.argument_keyframe_lerp(),
        }
    }
}
