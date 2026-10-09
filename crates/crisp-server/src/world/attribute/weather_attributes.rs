use std::sync::LazyLock;

use super::environment_attributes as attributes;
use super::modifier::{AttributeModifier as M, BlendToGray, ColorModifier, FloatModifier, FloatWithAlpha};
use super::{EnvironmentAttributeMap, Value};
use crate::util::argb;
use crate::world::timeline::timelines;

fn blend_to_gray(brightness: f32, factor: f32) -> Value {
    Value::BlendToGray(BlendToGray { brightness, factor })
}

fn alpha_blend(value: f32, alpha: f32) -> Value {
    Value::FloatWithAlpha(FloatWithAlpha { value, alpha })
}

fn weather_map(sky: (f32, f32), tint: (f32, f32), cloud: (f32, f32), level_alpha: f32) -> EnvironmentAttributeMap {
    let tint = argb::color_from_float(1.0, tint.0, tint.0, tint.1);
    EnvironmentAttributeMap::default()
        .modify(attributes::SKY_COLOR, M::Color(ColorModifier::BlendToGrayRgb), blend_to_gray(sky.0, sky.1))
        .modify(attributes::FOG_COLOR, M::Color(ColorModifier::MultiplyRgb), Value::Rgb(argb::vector3f_from_rgb24(tint)))
        .modify(attributes::CLOUD_COLOR, M::Color(ColorModifier::BlendToGrayArgb), blend_to_gray(cloud.0, cloud.1))
        .modify(attributes::SKY_LIGHT_LEVEL, M::Float(FloatModifier::AlphaBlend), alpha_blend(4.0, level_alpha))
        .modify(
            attributes::SKY_LIGHT_COLOR,
            M::Color(ColorModifier::AlphaBlendRgb),
            Value::Argb(argb::vector4f_from_argb32(argb::color_with_alpha(level_alpha, timelines::night_sky_light_color()))),
        )
        .modify(attributes::SKY_LIGHT_FACTOR, M::Float(FloatModifier::AlphaBlend), alpha_blend(0.24, level_alpha))
        .set(attributes::STAR_BRIGHTNESS, Value::Float(0.0))
        .modify(attributes::SUNRISE_SUNSET_COLOR, M::Color(ColorModifier::MultiplyArgb), Value::Argb(argb::vector4f_from_argb32(tint)))
        .set(attributes::BEES_STAY_IN_HIVE, Value::Boolean(true))
}

pub static RAIN: LazyLock<EnvironmentAttributeMap> = LazyLock::new(|| weather_map((0.6, 0.75), (0.5, 0.6), (0.24, 0.5), 0.3125));

pub static THUNDER: LazyLock<EnvironmentAttributeMap> = LazyLock::new(|| weather_map((0.24, 0.94), (0.25, 0.3), (0.095, 0.94), 0.52734375));
