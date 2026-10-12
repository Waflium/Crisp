use crisp_core::mth;

use super::{java_max_f, java_min_f};

fn clamp(value: f32) -> f32 {
    java_min_f(1.0, java_max_f(value, 0.0))
}

pub type Vec3f = [f32; 3];
pub type Vec4f = [f32; 4];

pub fn alpha(color: i32) -> i32 {
    (color as u32 >> 24) as i32
}

pub fn red(color: i32) -> i32 {
    color >> 16 & 0xFF
}

pub fn green(color: i32) -> i32 {
    color >> 8 & 0xFF
}

pub fn blue(color: i32) -> i32 {
    color & 0xFF
}

pub fn color(alpha: i32, red: i32, green: i32, blue: i32) -> i32 {
    (alpha & 0xFF) << 24 | (red & 0xFF) << 16 | (green & 0xFF) << 8 | blue & 0xFF
}

pub fn color_with_alpha(alpha: f32, rgb: i32) -> i32 {
    as_8bit_channel(alpha) << 24 | rgb & 0xFFFFFF
}

pub fn opaque(color: i32) -> i32 {
    color | 0xFF000000u32 as i32
}

pub fn average(lhs: i32, rhs: i32) -> i32 {
    color((alpha(lhs) + alpha(rhs)) / 2, (red(lhs) + red(rhs)) / 2, (green(lhs) + green(rhs)) / 2, (blue(lhs) + blue(rhs)) / 2)
}

pub fn as_8bit_channel(value: f32) -> i32 {
    mth::floor_f(value * 255.0)
}

fn from_8bit_channel(value: i32) -> f32 {
    value as f32 / 255.0
}

pub fn color_from_float(alpha: f32, red: f32, green: f32, blue: f32) -> i32 {
    color(as_8bit_channel(alpha), as_8bit_channel(red), as_8bit_channel(green), as_8bit_channel(blue))
}

pub fn vector3f_from_rgb24(color: i32) -> Vec3f {
    [from_8bit_channel(red(color)), from_8bit_channel(green(color)), from_8bit_channel(blue(color))]
}

pub fn vector4f_from_argb32(color: i32) -> Vec4f {
    [from_8bit_channel(red(color)), from_8bit_channel(green(color)), from_8bit_channel(blue(color)), from_8bit_channel(alpha(color))]
}

pub fn multiply3(lhs: Vec3f, rhs: Vec3f) -> Vec3f {
    if lhs == [1.0; 3] {
        rhs
    } else if rhs == [1.0; 3] {
        lhs
    } else {
        [lhs[0] * rhs[0], lhs[1] * rhs[1], lhs[2] * rhs[2]]
    }
}

pub fn multiply4(lhs: Vec4f, rhs: Vec4f) -> Vec4f {
    if lhs == [1.0; 4] {
        rhs
    } else if rhs == [1.0; 4] {
        lhs
    } else {
        [lhs[0] * rhs[0], lhs[1] * rhs[1], lhs[2] * rhs[2], lhs[3] * rhs[3]]
    }
}

pub fn add_rgb3(lhs: Vec3f, rhs: Vec3f) -> Vec3f {
    [java_min_f(lhs[0] + rhs[0], 1.0), java_min_f(lhs[1] + rhs[1], 1.0), java_min_f(lhs[2] + rhs[2], 1.0)]
}

pub fn add_rgb4(lhs: Vec4f, rhs: Vec3f) -> Vec4f {
    [java_min_f(lhs[0] + rhs[0], 1.0), java_min_f(lhs[1] + rhs[1], 1.0), java_min_f(lhs[2] + rhs[2], 1.0), lhs[3]]
}

pub fn subtract_rgb3(lhs: Vec3f, rhs: Vec3f) -> Vec3f {
    [java_max_f(lhs[0] - rhs[0], 0.0), java_max_f(lhs[1] - rhs[1], 0.0), java_max_f(lhs[2] - rhs[2], 0.0)]
}

pub fn subtract_rgb4(lhs: Vec4f, rhs: Vec3f) -> Vec4f {
    [java_max_f(lhs[0] - rhs[0], 0.0), java_max_f(lhs[1] - rhs[1], 0.0), java_max_f(lhs[2] - rhs[2], 0.0), lhs[3]]
}

pub fn scale_rgb3(color: Vec3f, scale: f32) -> Vec3f {
    [clamp(color[0] * scale), clamp(color[1] * scale), clamp(color[2] * scale)]
}

pub fn scale_rgb4(color: Vec4f, scale: f32) -> Vec4f {
    [clamp(color[0] * scale), clamp(color[1] * scale), clamp(color[2] * scale), color[3]]
}

pub fn greyscale3(color: Vec3f) -> Vec3f {
    let grey = color[0] * 0.3 + color[1] * 0.59 + color[2] * 0.11;
    [grey; 3]
}

pub fn greyscale4(color: Vec4f) -> Vec4f {
    let grey = color[0] * 0.3 + color[1] * 0.59 + color[2] * 0.11;
    [grey, grey, grey, color[3]]
}

pub fn alpha_blend3(destination: Vec3f, source: Vec4f) -> Vec3f {
    let source_alpha = source[3];
    if source_alpha == 0.0 {
        return destination;
    }
    srgb_lerp3(source_alpha, destination, [source[0], source[1], source[2]])
}

pub fn alpha_blend4(destination: Vec4f, source: Vec4f) -> Vec4f {
    let destination_alpha = destination[3];
    let source_alpha = source[3];
    if source_alpha == 1.0 {
        return source;
    }
    if source_alpha == 0.0 {
        return destination;
    }
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    let channel = |d: f32, s: f32| (s * source_alpha + d * (alpha - source_alpha)) / alpha;
    [channel(destination[0], source[0]), channel(destination[1], source[1]), channel(destination[2], source[2]), alpha]
}

pub fn srgb_lerp3(alpha: f32, p0: Vec3f, p1: Vec3f) -> Vec3f {
    std::array::from_fn(|i| (p1[i] - p0[i]) * alpha + p0[i])
}

pub fn srgb_lerp4(alpha: f32, p0: Vec4f, p1: Vec4f) -> Vec4f {
    std::array::from_fn(|i| (p1[i] - p0[i]) * alpha + p0[i])
}
