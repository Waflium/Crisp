use std::sync::Arc;

use crisp_core::mth;

use super::modifier::{BlendToGray, FloatWithAlpha};
use super::{AmbientParticle, Value};
use crate::util::argb;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LerpFunction {
    Float,
    Integer,
    Degrees(f32),
    Step(f32),
    ColorVec3,
    ColorVec4,
    AmbientParticleCrossFade,
    FloatWithAlpha,
    BlendToGray,
}

fn lerp_int(alpha: f32, p0: i32, p1: i32) -> i32 {
    p0.wrapping_add(mth::floor_f(alpha * p1.wrapping_sub(p0) as f32))
}

impl LerpFunction {
    pub const CONSTANT: LerpFunction = LerpFunction::Step(1.0);

    pub fn apply(self, alpha: f32, from: &Value, to: &Value) -> Value {
        match self {
            LerpFunction::Float => Value::Float(mth::lerp_f(alpha, from.as_float(), to.as_float())),
            LerpFunction::Integer => Value::Integer(lerp_int(alpha, from.as_int(), to.as_int())),
            LerpFunction::Degrees(max_delta) => {
                let (from, to) = (from.as_float(), to.as_float());
                let delta = mth::wrap_degrees(to - from);
                Value::Float(if delta.abs() >= max_delta { to } else { from + alpha * delta })
            }
            LerpFunction::Step(threshold) => if alpha >= threshold { to } else { from }.clone(),
            LerpFunction::ColorVec3 => Value::Rgb(argb::srgb_lerp3(alpha, from.as_rgb(), to.as_rgb())),
            LerpFunction::ColorVec4 => Value::Argb(argb::srgb_lerp4(alpha, from.as_argb(), to.as_argb())),
            LerpFunction::AmbientParticleCrossFade => {
                if alpha == 0.0 {
                    return from.clone();
                }
                if alpha == 1.0 {
                    return to.clone();
                }
                let (Value::AmbientParticles(from), Value::AmbientParticles(to)) = (from, to) else { panic!("not ambient particles") };
                let scale = |p: &AmbientParticle, alpha: f32| AmbientParticle { particle: p.particle.clone(), probability: p.probability * alpha };
                let list = from.iter().map(|p| scale(p, 1.0 - alpha)).chain(to.iter().map(|p| scale(p, alpha))).collect();
                Value::AmbientParticles(Arc::new(list))
            }
            LerpFunction::FloatWithAlpha => {
                let (Value::FloatWithAlpha(from), Value::FloatWithAlpha(to)) = (from, to) else { panic!("not FloatWithAlpha") };
                Value::FloatWithAlpha(FloatWithAlpha { value: mth::lerp_f(alpha, from.value, to.value), alpha: mth::lerp_f(alpha, from.alpha, to.alpha) })
            }
            LerpFunction::BlendToGray => {
                let (Value::BlendToGray(from), Value::BlendToGray(to)) = (from, to) else { panic!("not BlendToGray") };
                Value::BlendToGray(BlendToGray {
                    brightness: mth::lerp_f(alpha, from.brightness, to.brightness),
                    factor: mth::lerp_f(alpha, from.factor, to.factor),
                })
            }
        }
    }
}
