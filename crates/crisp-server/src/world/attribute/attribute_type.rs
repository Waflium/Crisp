use std::sync::Arc;

use serde_json::Value as Json;

use super::modifier::color_modifier::{parse_argb, parse_rgb};
use super::modifier::{AttributeModifier, BooleanModifier, ColorModifier, FloatModifier, IntegerModifier, OperationId};
use super::{AmbientParticle, BedRule, LerpFunction, Value};
use crate::util::tri_state::TriState;
use crate::world::entity::schedule::Activity;
use crate::world::level::biome::mob_spawn_settings::MobSpawnSettings;
use crate::world::level::levelgen::density_function::{float, resource_name};
use crate::world::level::moon_phase::MoonPhase;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AttributeType {
    Boolean,
    TriState,
    Float,
    AngleDegrees,
    RgbColor,
    ArgbColor,
    Integer,
    MoonPhase,
    Activity,
    BedRule,
    Particle,
    AmbientParticles,
    BackgroundMusic,
    AmbientSounds,
    MobSpawnSettings,
}

impl AttributeType {
    pub const ALL: [AttributeType; 15] = [
        AttributeType::Boolean,
        AttributeType::TriState,
        AttributeType::Float,
        AttributeType::AngleDegrees,
        AttributeType::RgbColor,
        AttributeType::ArgbColor,
        AttributeType::Integer,
        AttributeType::MoonPhase,
        AttributeType::Activity,
        AttributeType::BedRule,
        AttributeType::Particle,
        AttributeType::AmbientParticles,
        AttributeType::BackgroundMusic,
        AttributeType::AmbientSounds,
        AttributeType::MobSpawnSettings,
    ];

    pub fn name(self) -> &'static str {
        [
            "minecraft:boolean",
            "minecraft:tri_state",
            "minecraft:float",
            "minecraft:angle_degrees",
            "minecraft:rgb_color",
            "minecraft:argb_color",
            "minecraft:integer",
            "minecraft:moon_phase",
            "minecraft:activity",
            "minecraft:bed_rule",
            "minecraft:particle",
            "minecraft:ambient_particles",
            "minecraft:background_music",
            "minecraft:ambient_sounds",
            "minecraft:mob_spawn_settings",
        ][self as usize]
    }

    fn lerp(self) -> Option<LerpFunction> {
        match self {
            AttributeType::Float | AttributeType::AngleDegrees => Some(LerpFunction::Float),
            AttributeType::Integer => Some(LerpFunction::Integer),
            AttributeType::RgbColor => Some(LerpFunction::ColorVec3),
            AttributeType::ArgbColor => Some(LerpFunction::ColorVec4),
            AttributeType::AmbientParticles => Some(LerpFunction::AmbientParticleCrossFade),
            _ => None,
        }
    }

    pub fn keyframe_lerp(self) -> LerpFunction {
        self.lerp().unwrap_or(LerpFunction::Step(1.0))
    }

    pub fn state_change_lerp(self) -> LerpFunction {
        self.lerp().unwrap_or(LerpFunction::Step(0.0))
    }

    pub fn spatial_lerp(self) -> LerpFunction {
        self.lerp().unwrap_or(LerpFunction::Step(0.5))
    }

    pub fn partial_tick_lerp(self) -> LerpFunction {
        match self {
            AttributeType::AngleDegrees => LerpFunction::Degrees(90.0),
            _ => self.lerp().unwrap_or(LerpFunction::Step(0.0)),
        }
    }

    pub fn modifier(self, operation: OperationId) -> Option<AttributeModifier> {
        use OperationId as Op;
        if operation == Op::Override {
            return Some(AttributeModifier::Override);
        }
        let modifier = match (self, operation) {
            (AttributeType::Boolean, Op::And) => AttributeModifier::Boolean(BooleanModifier::And),
            (AttributeType::Boolean, Op::Nand) => AttributeModifier::Boolean(BooleanModifier::Nand),
            (AttributeType::Boolean, Op::Or) => AttributeModifier::Boolean(BooleanModifier::Or),
            (AttributeType::Boolean, Op::Nor) => AttributeModifier::Boolean(BooleanModifier::Nor),
            (AttributeType::Boolean, Op::Xor) => AttributeModifier::Boolean(BooleanModifier::Xor),
            (AttributeType::Boolean, Op::Xnor) => AttributeModifier::Boolean(BooleanModifier::Xnor),
            (AttributeType::Float | AttributeType::AngleDegrees, op) => AttributeModifier::Float(match op {
                Op::AlphaBlend => FloatModifier::AlphaBlend,
                Op::Add => FloatModifier::Add,
                Op::Subtract => FloatModifier::Subtract,
                Op::Multiply => FloatModifier::Multiply,
                Op::Minimum => FloatModifier::Minimum,
                Op::Maximum => FloatModifier::Maximum,
                _ => return None,
            }),
            (AttributeType::RgbColor, op) => AttributeModifier::Color(match op {
                Op::AlphaBlend => ColorModifier::AlphaBlendRgb,
                Op::Add => ColorModifier::AddRgb,
                Op::Subtract => ColorModifier::SubtractRgb,
                Op::Multiply => ColorModifier::MultiplyRgb,
                Op::BlendToGray => ColorModifier::BlendToGrayRgb,
                _ => return None,
            }),
            (AttributeType::ArgbColor, op) => AttributeModifier::Color(match op {
                Op::AlphaBlend => ColorModifier::AlphaBlendArgb,
                Op::Add => ColorModifier::AddArgb,
                Op::Subtract => ColorModifier::SubtractArgb,
                Op::Multiply => ColorModifier::MultiplyArgb,
                Op::BlendToGray => ColorModifier::BlendToGrayArgb,
                _ => return None,
            }),
            (AttributeType::Integer, op) => AttributeModifier::Integer(match op {
                Op::Add => IntegerModifier::Add,
                Op::Subtract => IntegerModifier::Subtract,
                Op::Multiply => IntegerModifier::Multiply,
                Op::Minimum => IntegerModifier::Minimum,
                Op::Maximum => IntegerModifier::Maximum,
                _ => return None,
            }),
            (AttributeType::AmbientParticles, Op::Append) => AttributeModifier::Append,
            (AttributeType::MobSpawnSettings, Op::Overlay) => AttributeModifier::Overlay,
            _ => return None,
        };
        Some(modifier)
    }

    pub fn parse_value(self, json: &Json) -> Result<Value, String> {
        Ok(match self {
            AttributeType::Boolean => Value::Boolean(super::parse_bool(json)?),
            AttributeType::TriState => Value::TriState(TriState::parse(json)?),
            AttributeType::Float | AttributeType::AngleDegrees => Value::Float(float(json)?),
            AttributeType::RgbColor => Value::Rgb(parse_rgb(json)?),
            AttributeType::ArgbColor => Value::Argb(parse_argb(json)?),
            AttributeType::Integer => Value::Integer(super::parse_int(json)?),
            AttributeType::MoonPhase => Value::MoonPhase(json.as_str().and_then(MoonPhase::by_name).ok_or_else(|| format!("unknown moon phase {json}"))?),
            AttributeType::Activity => {
                let name = json.as_str().ok_or_else(|| format!("not an activity: {json}"))?;
                Value::Activity(Activity::by_name(&resource_name(name)).ok_or_else(|| format!("unknown activity {name}"))?)
            }
            AttributeType::BedRule => Value::BedRule(Arc::new(BedRule::parse(json)?)),
            AttributeType::Particle | AttributeType::BackgroundMusic | AttributeType::AmbientSounds => Value::Data(Arc::new(json.clone())),
            AttributeType::AmbientParticles => {
                let list = json.as_array().ok_or_else(|| format!("not a list: {json}"))?;
                Value::AmbientParticles(Arc::new(list.iter().map(AmbientParticle::parse).collect::<Result<_, _>>()?))
            }
            AttributeType::MobSpawnSettings => Value::MobSpawnSettings(Arc::new(MobSpawnSettings::parse(json)?)),
        })
    }

    pub fn to_float(self, value: &Value) -> Option<f32> {
        match (self, value) {
            (AttributeType::Float | AttributeType::AngleDegrees, Value::Float(f)) => Some(*f),
            (AttributeType::Integer, Value::Integer(i)) => Some(*i as f32),
            _ => None,
        }
    }

    pub fn to_int(self, value: &Value) -> Option<i32> {
        match (self, value) {
            (AttributeType::Integer, Value::Integer(i)) => Some(*i),
            _ => None,
        }
    }
}
