use std::sync::LazyLock;

use crate::random::RandomSource;

pub const PI: f32 = std::f64::consts::PI as f32;
pub const DEG_TO_RAD: f32 = (std::f64::consts::PI / 180.0) as f32;
pub const RAD_TO_DEG: f32 = 180.0 / std::f64::consts::PI as f32;

const SIN_SCALE: f64 = 10430.378350470453;

static SIN: LazyLock<Box<[f32; 65536]>> = LazyLock::new(|| {
    let mut table = Box::new([0.0; 65536]);
    for (i, v) in table.iter_mut().enumerate() {
        *v = (i as f64 / SIN_SCALE).sin() as f32;
    }
    table
});

pub fn sin(i: f64) -> f32 {
    SIN[((i * SIN_SCALE) as i64 & 0xFFFF) as usize]
}

pub fn cos(i: f64) -> f32 {
    SIN[((i * SIN_SCALE + 16384.0) as i64 & 0xFFFF) as usize]
}

pub fn sqrt(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

pub fn java_round(x: f64) -> i64 {
    let f = x.floor();
    if x - f >= 0.5 { f as i64 + 1 } else { f as i64 }
}

pub fn java_round_f32(x: f32) -> i32 {
    let f = x.floor();
    if x - f >= 0.5 { (f as i32).saturating_add(1) } else { f as i32 }
}

pub fn floor(v: f64) -> i32 {
    v.floor() as i32
}

pub fn floor_f(v: f32) -> i32 {
    v.floor() as i32
}

pub fn lfloor(v: f64) -> i64 {
    v.floor() as i64
}

pub fn ceil(v: f64) -> i32 {
    v.ceil() as i32
}

pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min { min } else { value.min(max) }
}

pub fn clamp_f(value: f32, min: f32, max: f32) -> f32 {
    if value < min { min } else { value.min(max) }
}

pub fn clamp_i(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

pub fn jmin(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
}

pub fn jmax(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        b
    } else if a >= b {
        a
    } else {
        b
    }
}

pub fn jmin_f(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
}

pub fn jmax_f(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        b
    } else if a >= b {
        a
    } else {
        b
    }
}

pub fn jclamp_f(value: f32, min: f32, max: f32) -> f32 {
    if value < min { min } else { jmin_f(value, max) }
}

pub fn jclamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min { min } else { jmin(value, max) }
}

pub fn java_signum_f(x: f32) -> f32 {
    if x == 0.0 || x.is_nan() { x } else { 1.0f32.copysign(x) }
}

pub fn smoothstep(x: f32) -> f32 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

#[allow(clippy::too_many_arguments)]
pub fn lerp3_f(a1: f32, a2: f32, a3: f32, x000: f32, x100: f32, x010: f32, x110: f32, x001: f32, x101: f32, x011: f32, x111: f32) -> f32 {
    lerp_f(a3, lerp_f(a2, lerp_f(a1, x000, x100), lerp_f(a1, x010, x110)), lerp_f(a2, lerp_f(a1, x001, x101), lerp_f(a1, x011, x111)))
}

pub fn lerp(alpha: f64, p0: f64, p1: f64) -> f64 {
    p0 + alpha * (p1 - p0)
}

pub fn lerp_f(alpha: f32, p0: f32, p1: f32) -> f32 {
    p0 + alpha * (p1 - p0)
}

pub fn clamped_lerp(factor: f64, min: f64, max: f64) -> f64 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp(factor, min, max)
    }
}

pub fn wrap_degrees(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

pub fn wrap_degrees_d(angle: f64) -> f64 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

pub fn wrap_degrees_i(angle: i32) -> i32 {
    let mut a = angle % 360;
    if a >= 180 {
        a -= 360;
    }
    if a < -180 {
        a += 360;
    }
    a
}

pub fn degrees_difference(from: f32, to: f32) -> f32 {
    wrap_degrees(to - from)
}

pub fn rotate_if_necessary(base: f32, target: f32, max_diff: f32) -> f32 {
    let delta = degrees_difference(base, target);
    target - clamp_f(delta, -max_diff, max_diff)
}

pub fn approach(current: f32, target: f32, increment: f32) -> f32 {
    let increment = increment.abs();
    if current < target { clamp_f(current + increment, current, target) } else { clamp_f(current - increment, target, current) }
}

pub fn approach_degrees(current: f32, target: f32, increment: f32) -> f32 {
    approach(current, current + degrees_difference(current, target), increment)
}

pub fn pack_degrees(angle: f32) -> i8 {
    floor_f(angle * 256.0 / 360.0) as i8
}

pub fn unpack_degrees(rot: i8) -> f32 {
    (rot as i32 * 360) as f32 / 256.0
}

pub fn positive_modulo(input: i32, m: i32) -> i32 {
    input.rem_euclid(m)
}

pub fn positive_modulo_d(input: f64, m: f64) -> f64 {
    (input % m + m) % m
}

pub fn frac(num: f64) -> f64 {
    num - lfloor(num) as f64
}

pub fn frac_f(num: f32) -> f32 {
    num - floor(num as f64) as f32
}

pub fn lerp_discrete(alpha: f32, p0: i32, p1: i32) -> i32 {
    let delta = p1 - p0;
    p0 + floor((alpha * (delta - 1) as f32) as f64) + if alpha > 0.0 { 1 } else { 0 }
}

pub fn sign(number: f64) -> i32 {
    if number == 0.0 {
        0
    } else if number > 0.0 {
        1
    } else {
        -1
    }
}

pub fn square(x: f64) -> f64 {
    x * x
}

pub fn abs_max(a: f64, b: f64) -> f64 {
    jmax(a.abs(), b.abs())
}

static ASIN_COS_TAB: LazyLock<[(f64, f64); 257]> = LazyLock::new(|| {
    let mut table = [(0.0, 0.0); 257];
    for (i, entry) in table.iter_mut().enumerate() {
        let asin = (i as f64 / 256.0).asin();
        *entry = (asin, asin.cos());
    }
    table
});

pub fn fast_inv_sqrt(x: f64) -> f64 {
    let half = 0.5 * x;
    let i = 6910469410427058090i64.wrapping_sub((x.to_bits() as i64) >> 1);
    let x = f64::from_bits(i as u64);
    x * (1.5 - half * x * x)
}

pub fn atan2(mut y: f64, mut x: f64) -> f64 {
    let d2 = x * x + y * y;
    if d2.is_nan() {
        return f64::NAN;
    }
    let neg_y = y < 0.0;
    if neg_y {
        y = -y;
    }
    let neg_x = x < 0.0;
    if neg_x {
        x = -x;
    }
    let steep = y > x;
    if steep {
        std::mem::swap(&mut x, &mut y);
    }
    let rinv = fast_inv_sqrt(d2);
    x *= rinv;
    y *= rinv;
    let frac_bias = f64::from_bits(4805340802404319232);
    let yp = frac_bias + y;
    let index = yp.to_bits() as i32 as usize;
    let (phi, c_phi) = ASIN_COS_TAB[index];
    let s_phi = yp - frac_bias;
    let sd = y * c_phi - x * s_phi;
    let d = (6.0 + sd * sd) * sd * 0.16666666666666666;
    let mut theta = phi + d;
    if steep {
        theta = std::f64::consts::FRAC_PI_2 - theta;
    }
    if neg_x {
        theta = std::f64::consts::PI - theta;
    }
    if neg_y {
        theta = -theta;
    }
    theta
}

pub fn length_squared(x: f64, y: f64, z: f64) -> f64 {
    x * x + y * y + z * z
}

pub fn length(x: f64, y: f64, z: f64) -> f64 {
    length_squared(x, y, z).sqrt()
}

pub fn equal(a: f64, b: f64) -> bool {
    (b - a).abs() < 1.0e-5f32 as f64
}

pub fn smallest_encompassing_power_of_two(input: i32) -> i32 {
    let mut r = input - 1;
    r |= r >> 1;
    r |= r >> 2;
    r |= r >> 4;
    r |= r >> 8;
    r |= r >> 16;
    r + 1
}

pub fn is_power_of_two(input: i32) -> bool {
    input != 0 && input & (input - 1) == 0
}

pub fn ceillog2(input: i32) -> i32 {
    const DE_BRUIJN: [i32; 32] = [0, 1, 28, 2, 29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4, 8, 31, 27, 13, 23, 21, 19, 16, 7, 26, 12, 18, 6, 11, 5, 10, 9];
    let input = if is_power_of_two(input) { input } else { smallest_encompassing_power_of_two(input) };
    DE_BRUIJN[((input as i64 * 125613361 >> 27) & 31) as usize]
}

pub fn log2(input: i32) -> i32 {
    ceillog2(input) - if is_power_of_two(input) { 0 } else { 1 }
}

pub fn get_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut seed = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ y as i64;
    seed = seed.wrapping_mul(seed).wrapping_mul(42317861).wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

pub fn next_int(random: &mut impl RandomSource, min: i32, max: i32) -> i32 {
    if min >= max { min } else { random.next_int_bounded(max - min + 1) + min }
}

pub fn next_float(random: &mut impl RandomSource, min: f32, max: f32) -> f32 {
    if min >= max { min } else { random.next_float() * (max - min) + min }
}

pub fn next_double(random: &mut impl RandomSource, min: f64, max: f64) -> f64 {
    if min >= max { min } else { random.next_double() * (max - min) + min }
}

pub fn random_between_inclusive(random: &mut impl RandomSource, min: i32, max_inclusive: i32) -> i32 {
    random.next_int_bounded(max_inclusive - min + 1) + min
}

pub fn random_between(random: &mut impl RandomSource, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

pub fn inverse_lerp(value: f64, min: f64, max: f64) -> f64 {
    (value - min) / (max - min)
}

pub fn clamped_map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    clamped_lerp(inverse_lerp(value, from_min, from_max), to_min, to_max)
}

pub fn clamped_lerp_f(factor: f32, min: f32, max: f32) -> f32 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp_f(factor, min, max)
    }
}

pub fn inverse_lerp_f(value: f32, min: f32, max: f32) -> f32 {
    (value - min) / (max - min)
}

pub fn clamped_map_f(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    clamped_lerp_f(inverse_lerp_f(value, from_min, from_max), to_min, to_max)
}

pub fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    lerp(inverse_lerp(value, from_min, from_max), to_min, to_max)
}

pub fn quantize(value: f64, resolution: i32) -> i32 {
    floor(value / resolution as f64) * resolution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_seed_overflows_like_java() {
        assert_eq!(get_seed(0, 0, 0), 0);
        assert_eq!(get_seed(1, 2, 3), -33674130277896);
        assert_eq!(get_seed(-30000000, 64, 30000000), -35216720112214);
    }

    #[test]
    fn table_sine() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert_eq!(sin(1.0), 0.8414514);
        assert_eq!(cos(2.5), -0.80108935);
        assert_eq!(wrap_degrees(-725.5), -5.5);
    }

    #[test]
    fn logs() {
        assert_eq!(ceillog2(1), 0);
        assert_eq!(ceillog2(5), 3);
        assert_eq!(log2(30000000), 24);
        assert_eq!(smallest_encompassing_power_of_two(30000000), 33554432);
    }

    #[test]
    fn table_atan2() {
        assert_eq!(atan2(0.0, 5.0), 0.0);
        assert_eq!(atan2(3.0, -2.0), 2.1587989282661573);
        assert_eq!(atan2(-1.0, 7.0), -0.14189668178982134);
    }
}
