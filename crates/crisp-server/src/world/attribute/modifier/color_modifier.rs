use serde_json::Value as Json;

use crate::util::argb::{self, Vec3f, Vec4f};
use crate::world::attribute::{LerpFunction, Value};
use crate::world::level::levelgen::density_function::float;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ColorModifier {
    AlphaBlendRgb,
    AlphaBlendArgb,
    AddRgb,
    SubtractRgb,
    MultiplyRgb,
    AddArgb,
    SubtractArgb,
    MultiplyArgb,
    BlendToGrayRgb,
    BlendToGrayArgb,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BlendToGray {
    pub brightness: f32,
    pub factor: f32,
}

impl BlendToGray {
    pub fn parse(json: &Json) -> Result<BlendToGray, String> {
        let unit = |key: &str| {
            let v = float(json.get(key).ok_or_else(|| format!("missing {key}"))?)?;
            if (0.0..=1.0).contains(&v) { Ok(v) } else { Err(format!("Value {v} outside of range [0.0:1.0]")) }
        };
        Ok(BlendToGray { brightness: unit("brightness")?, factor: unit("factor")? })
    }
}

pub fn parse_rgb(json: &Json) -> Result<Vec3f, String> {
    if let Some(s) = json.as_str() {
        return hex_color(s, 6).map(|c| argb::vector3f_from_rgb24(argb::opaque(c)));
    }
    if let Some(list) = json.as_array() {
        let [r, g, b] = floats(list)?.try_into().map_err(|l: Vec<f32>| format!("Input is not a list of 3 elements: {l:?}"))?;
        return Ok([r, g, b]);
    }
    crate::world::attribute::parse_int(json).map(argb::vector3f_from_rgb24)
}

pub fn parse_argb(json: &Json) -> Result<Vec4f, String> {
    if let Some(s) = json.as_str() {
        return hex_color(s, 8).map(argb::vector4f_from_argb32);
    }
    if let Some(list) = json.as_array() {
        let [r, g, b, a] = floats(list)?.try_into().map_err(|l: Vec<f32>| format!("Input is not a list of 4 elements: {l:?}"))?;
        return Ok([r, g, b, a]);
    }
    crate::world::attribute::parse_int(json).map(argb::vector4f_from_argb32)
}

fn floats(list: &[Json]) -> Result<Vec<f32>, String> {
    list.iter().map(float).collect()
}

fn hex_color(s: &str, digits: usize) -> Result<i32, String> {
    let hex = s.strip_prefix('#').ok_or("Hex color must begin with #")?;
    if hex.len() != digits {
        return Err(format!("Hex color is wrong size, expected {digits} digits but got {}", hex.len()));
    }
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("Invalid color value: {s}"));
    }
    Ok(u32::from_str_radix(hex, 16).expect("hex digits") as i32)
}

impl ColorModifier {
    pub fn apply(self, subject: &Value, argument: &Value) -> Value {
        match self {
            ColorModifier::AlphaBlendRgb => Value::Rgb(argb::alpha_blend3(subject.as_rgb(), argument.as_argb())),
            ColorModifier::AlphaBlendArgb => Value::Argb(argb::alpha_blend4(subject.as_argb(), argument.as_argb())),
            ColorModifier::AddRgb => Value::Rgb(argb::add_rgb3(subject.as_rgb(), argument.as_rgb())),
            ColorModifier::SubtractRgb => Value::Rgb(argb::subtract_rgb3(subject.as_rgb(), argument.as_rgb())),
            ColorModifier::MultiplyRgb => Value::Rgb(argb::multiply3(subject.as_rgb(), argument.as_rgb())),
            ColorModifier::AddArgb => Value::Argb(argb::add_rgb4(subject.as_argb(), argument.as_rgb())),
            ColorModifier::SubtractArgb => Value::Argb(argb::subtract_rgb4(subject.as_argb(), argument.as_rgb())),
            ColorModifier::MultiplyArgb => Value::Argb(argb::multiply4(subject.as_argb(), argument.as_argb())),
            ColorModifier::BlendToGrayRgb => {
                let Value::BlendToGray(argument) = argument else { panic!("{argument:?} is not BlendToGray") };
                let subject = subject.as_rgb();
                let grey = argb::scale_rgb3(argb::greyscale3(subject), argument.brightness);
                Value::Rgb(argb::srgb_lerp3(argument.factor, subject, grey))
            }
            ColorModifier::BlendToGrayArgb => {
                let Value::BlendToGray(argument) = argument else { panic!("{argument:?} is not BlendToGray") };
                let subject = subject.as_argb();
                let grey = argb::scale_rgb4(argb::greyscale4(subject), argument.brightness);
                Value::Argb(argb::srgb_lerp4(argument.factor, subject, grey))
            }
        }
    }

    pub fn parse_argument(self, json: &Json) -> Result<Value, String> {
        match self {
            ColorModifier::AlphaBlendRgb | ColorModifier::AlphaBlendArgb | ColorModifier::MultiplyArgb => parse_argb(json).map(Value::Argb),
            ColorModifier::BlendToGrayRgb | ColorModifier::BlendToGrayArgb => BlendToGray::parse(json).map(Value::BlendToGray),
            _ => parse_rgb(json).map(Value::Rgb),
        }
    }

    pub fn argument_keyframe_lerp(self) -> LerpFunction {
        match self {
            ColorModifier::AlphaBlendRgb | ColorModifier::AlphaBlendArgb | ColorModifier::MultiplyArgb => LerpFunction::ColorVec4,
            ColorModifier::BlendToGrayRgb | ColorModifier::BlendToGrayArgb => LerpFunction::BlendToGray,
            _ => LerpFunction::ColorVec3,
        }
    }
}
