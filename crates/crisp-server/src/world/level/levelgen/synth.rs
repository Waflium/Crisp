use crisp_core::mth::{self, lerp3_f, smoothstep};
use crisp_core::{Interval, RandomSource};

use super::density_function::DensityVolume;
use super::AlgorithmRandom;

const GRADIENT: [(i32, i32, i32); 16] = [
    (1, 1, 0),
    (-1, 1, 0),
    (1, -1, 0),
    (-1, -1, 0),
    (1, 0, 1),
    (-1, 0, 1),
    (1, 0, -1),
    (-1, 0, -1),
    (0, 1, 1),
    (0, -1, 1),
    (0, 1, -1),
    (0, -1, -1),
    (1, 1, 0),
    (0, -1, 1),
    (-1, 1, 0),
    (0, -1, -1),
];

const HALF_ROUND_OFF: f64 = 16777215.999999998;

fn wrap(x: f64) -> f64 {
    if (-HALF_ROUND_OFF..HALF_ROUND_OFF).contains(&x) { x } else { x - (x / 3.3554432E7 + 0.5).floor() * 3.3554432E7 }
}

fn grad_dot(hash: i32, x: f32, y: f32, z: f32) -> f32 {
    let (gx, gy, gz) = GRADIENT[(hash & 15) as usize];
    gx as f32 * x + gy as f32 * y + gz as f32 * z
}

#[derive(Clone)]
struct GradientNoise {
    perms: [u8; 256],
    offset_x: f64,
    offset_y: f64,
    offset_z: f64,
}

impl GradientNoise {
    fn new(random: &mut impl RandomSource, offset_scale: f64) -> GradientNoise {
        let offset_x = random.next_double() * offset_scale;
        let offset_y = random.next_double() * offset_scale;
        let offset_z = random.next_double() * offset_scale;
        let mut perms = [0u8; 256];
        for (i, p) in perms.iter_mut().enumerate() {
            *p = i as u8;
        }
        for i in 0..256 {
            let offset = random.next_int_bounded(256 - i as i32) as usize;
            perms.swap(i, offset + i);
        }
        GradientNoise { perms, offset_x, offset_y, offset_z }
    }

    #[inline(always)]
    fn permute(&self, x: i32) -> i32 {
        self.perms[(x & 0xFF) as usize] as i32
    }
}

#[derive(Clone)]
pub struct PerlinNoise {
    g: GradientNoise,
    fudge_y_scale: Option<f64>,
}

impl PerlinNoise {
    pub fn new(random: &mut impl RandomSource) -> PerlinNoise {
        PerlinNoise { g: GradientNoise::new(random, 256.0), fudge_y_scale: None }
    }

    pub fn new_smeared(random: &mut impl RandomSource, fudge_y_scale: f64) -> PerlinNoise {
        PerlinNoise { g: GradientNoise::new(random, 256.0), fudge_y_scale: Some(fudge_y_scale) }
    }

    pub fn range(&self) -> Interval {
        match self.fudge_y_scale {
            None => Interval::of_symmetric(2.0),
            Some(scale) => smeared_range(scale),
        }
    }

    fn fudge_y(scale: f64, original_y: f64, relative_y: f64) -> f64 {
        let limit = if original_y >= 0.0 && original_y < relative_y { original_y } else { relative_y };
        mth::floor(limit / scale + 1.0E-7f32 as f64) as f64 * scale
    }

    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let g = &self.g;
        let px = wrap(x) + g.offset_x;
        let py = wrap(y) + g.offset_y;
        let pz = wrap(z) + g.offset_z;
        let floor_x = mth::floor(px);
        let floor_y = mth::floor(py);
        let floor_z = mth::floor(pz);
        let relative_x = (px - floor_x as f64) as f32;
        let relative_z = (pz - floor_z as f64) as f32;
        match self.fudge_y_scale {
            None => {
                let relative_y = (py - floor_y as f64) as f32;
                self.sample_and_lerp(floor_x, floor_y, floor_z, relative_x, relative_y, relative_z, relative_y)
            }
            Some(scale) => {
                let relative_y = py - floor_y as f64;
                let fudged = (relative_y - Self::fudge_y(scale, y, relative_y)) as f32;
                self.sample_and_lerp(floor_x, floor_y, floor_z, relative_x, fudged, relative_z, relative_y as f32)
            }
        }
    }

    pub fn get_2d(&self, x: f64, y: f64) -> f32 {
        self.get(wrap(x), 0.0, wrap(y))
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn sample_and_lerp(&self, x: i32, y: i32, z: i32, rx: f32, ry: f32, rz: f32, original_ry: f32) -> f32 {
        let g = &self.g;
        let x0 = g.permute(x);
        let x1 = g.permute(x.wrapping_add(1));
        let xy00 = g.permute(x0 + y);
        let xy01 = g.permute(x0 + y + 1);
        let xy10 = g.permute(x1 + y);
        let xy11 = g.permute(x1 + y + 1);
        let d000 = grad_dot(g.permute(xy00 + z), rx, ry, rz);
        let d100 = grad_dot(g.permute(xy10 + z), rx - 1.0, ry, rz);
        let d010 = grad_dot(g.permute(xy01 + z), rx, ry - 1.0, rz);
        let d110 = grad_dot(g.permute(xy11 + z), rx - 1.0, ry - 1.0, rz);
        let d001 = grad_dot(g.permute(xy00 + z + 1), rx, ry, rz - 1.0);
        let d101 = grad_dot(g.permute(xy10 + z + 1), rx - 1.0, ry, rz - 1.0);
        let d011 = grad_dot(g.permute(xy01 + z + 1), rx, ry - 1.0, rz - 1.0);
        let d111 = grad_dot(g.permute(xy11 + z + 1), rx - 1.0, ry - 1.0, rz - 1.0);
        lerp3_f(smoothstep(rx), smoothstep(original_ry), smoothstep(rz), d000, d100, d010, d110, d001, d101, d011, d111)
    }

    pub fn add_to_volume(&self, buffer: &mut [f32], volume: &DensityVolume, xz_scale: f64, y_scale: f64, amplitude: f32) {
        let g = &self.g;
        let mut d_xz = [0.0f32; 8];
        let mut g_y = [0.0f32; 8];
        let mut index = 0;
        for iz in 0..volume.size_z {
            let z = wrap(volume.block_z(iz) as f64 * xz_scale) + g.offset_z;
            let floor_z = mth::floor(z);
            let relative_z = (z - floor_z as f64) as f32;
            let alpha_z = smoothstep(relative_z);
            for ix in 0..volume.size_x {
                let x = wrap(volume.block_x(ix) as f64 * xz_scale) + g.offset_x;
                let floor_x = mth::floor(x);
                let relative_x = (x - floor_x as f64) as f32;
                let x0 = g.permute(floor_x);
                let x1 = g.permute(floor_x.wrapping_add(1));
                let alpha_x = smoothstep(relative_x);
                let mut last_floor_y = i32::MIN;
                for iy in 0..volume.size_y {
                    let original_y = volume.block_y(iy) as f64 * y_scale;
                    let y = wrap(original_y) + g.offset_y;
                    let floor_y = mth::floor(y);
                    let relative_y = y - floor_y as f64;
                    let alpha_y = smoothstep(relative_y as f32);
                    if last_floor_y != floor_y {
                        let xy00 = g.permute(x0 + floor_y);
                        let xy01 = g.permute(x0 + floor_y + 1);
                        let xy10 = g.permute(x1 + floor_y);
                        let xy11 = g.permute(x1 + floor_y + 1);
                        let corners = [
                            (xy00 + floor_z, relative_x, relative_z),
                            (xy10 + floor_z, relative_x - 1.0, relative_z),
                            (xy01 + floor_z, relative_x, relative_z),
                            (xy11 + floor_z, relative_x - 1.0, relative_z),
                            (xy00 + floor_z + 1, relative_x, relative_z - 1.0),
                            (xy10 + floor_z + 1, relative_x - 1.0, relative_z - 1.0),
                            (xy01 + floor_z + 1, relative_x, relative_z - 1.0),
                            (xy11 + floor_z + 1, relative_x - 1.0, relative_z - 1.0),
                        ];
                        for (i, &(hash, cx, cz)) in corners.iter().enumerate() {
                            let (gx, gy, gz) = GRADIENT[(g.permute(hash) & 15) as usize];
                            d_xz[i] = gx as f32 * cx + gz as f32 * cz;
                            g_y[i] = gy as f32;
                        }
                        last_floor_y = floor_y;
                    }
                    let ry = match self.fudge_y_scale {
                        None => relative_y as f32,
                        Some(scale) => (relative_y - Self::fudge_y(scale, original_y, relative_y)) as f32,
                    };
                    let value = lerp3_f(
                        alpha_x,
                        alpha_y,
                        alpha_z,
                        d_xz[0] + g_y[0] * ry,
                        d_xz[1] + g_y[1] * ry,
                        d_xz[2] + g_y[2] * (ry - 1.0),
                        d_xz[3] + g_y[3] * (ry - 1.0),
                        d_xz[4] + g_y[4] * ry,
                        d_xz[5] + g_y[5] * ry,
                        d_xz[6] + g_y[6] * (ry - 1.0),
                        d_xz[7] + g_y[7] * (ry - 1.0),
                    );
                    buffer[index] += amplitude * value;
                    index += 1;
                }
            }
        }
    }
}

fn smeared_range(fudge_y_scale: f64) -> Interval {
    Interval::of_symmetric((fudge_y_scale.abs() + 2.0) as f32)
}

#[derive(Clone)]
pub struct SimplexNoise {
    g: GradientNoise,
}

const SQRT_3: f64 = 1.7320508075688772;
const F2: f64 = 0.5 * (SQRT_3 - 1.0);
const G2: f64 = (3.0 - SQRT_3) / 6.0;

impl SimplexNoise {
    pub fn new(random: &mut impl RandomSource, discard_offset: bool) -> SimplexNoise {
        SimplexNoise { g: GradientNoise::new(random, if discard_offset { 0.0 } else { 256.0 }) }
    }

    fn corner_noise(index: i32, x: f64, y: f64, z: f64, base: f64) -> f64 {
        let mut t0 = base - x * x - y * y - z * z;
        if t0 < 0.0 {
            0.0
        } else {
            t0 *= t0;
            let (gx, gy, gz) = GRADIENT[index as usize];
            t0 * t0 * (gx as f64 * x + gy as f64 * y + gz as f64 * z)
        }
    }

    pub fn get_2d(&self, x: f64, y: f64) -> f32 {
        let g = &self.g;
        let xin = x + g.offset_x;
        let yin = y + g.offset_y;
        let s = (xin + yin) * F2;
        let i = mth::floor(xin + s);
        let j = mth::floor(yin + s);
        let t = (i.wrapping_add(j)) as f64 * G2;
        let x0 = xin - (i as f64 - t);
        let y0 = yin - (j as f64 - t);
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + G2;
        let y1 = y0 - j1 as f64 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;
        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let gi0 = g.permute(ii + g.permute(jj)) % 12;
        let gi1 = g.permute(ii + i1 + g.permute(jj + j1)) % 12;
        let gi2 = g.permute(ii + 1 + g.permute(jj + 1)) % 12;
        let n0 = Self::corner_noise(gi0, x0, y0, 0.0, 0.5);
        let n1 = Self::corner_noise(gi1, x1, y1, 0.0, 0.5);
        let n2 = Self::corner_noise(gi2, x2, y2, 0.0, 0.5);
        (70.0 * (n0 + n1 + n2)) as f32
    }
}

#[derive(Clone)]
struct Layer {
    noise: PerlinNoise,
    frequency: f64,
    amplitude: f32,
}

#[derive(Clone)]
pub struct NoiseStack {
    layers: Vec<Layer>,
    range: Interval,
}

impl NoiseStack {
    fn new(layers: Vec<Layer>) -> NoiseStack {
        let mut range = Interval::of_exact(0.0);
        for layer in &layers {
            range = Interval::add(range, Interval::mul(layer.noise.range(), Interval::of_exact(layer.amplitude)));
        }
        NoiseStack { layers, range }
    }

    pub fn range(&self) -> Interval {
        self.range
    }

    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let mut value = 0.0f32;
        for layer in &self.layers {
            let f = layer.frequency;
            value += layer.amplitude * layer.noise.get(x * f, y * f, z * f);
        }
        value
    }

    pub fn get_2d(&self, x: f64, y: f64) -> f32 {
        let mut value = 0.0f32;
        for layer in &self.layers {
            let f = layer.frequency;
            value += layer.amplitude * layer.noise.get_2d(x * f, y * f);
        }
        value
    }

    pub fn add_to_volume(&self, buffer: &mut [f32], volume: &DensityVolume, xz_scale: f64, y_scale: f64, amplitude: f32) {
        for layer in &self.layers {
            let f = layer.frequency;
            layer.noise.add_to_volume(buffer, volume, xz_scale * f, y_scale * f, amplitude * layer.amplitude);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalNoiseParameters {
    pub base_amplitude: f64,
    pub base_octave: i32,
    pub octave_count: i32,
    pub normalize: Normalization,
    pub amplitude_modifiers: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Normalization {
    Disabled,
    Enabled,
    Legacy,
}

struct OctaveInfo {
    index: i32,
    frequency: f64,
    amplitude: f64,
}

pub struct NormalNoise {
    parameters: NormalNoiseParameters,
    octaves: Vec<OctaveInfo>,
    normalization_factor: f64,
    range: Interval,
}

fn java_double_sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut sum, mut compensation, mut simple) = (0.0f64, 0.0f64, 0.0f64);
    for value in values {
        let tmp = value - compensation;
        let velvel = sum + tmp;
        compensation = (velvel - sum) - tmp;
        sum = velvel;
        simple += value;
    }
    let tmp = sum - compensation;
    if tmp.is_nan() && simple.is_infinite() { simple } else { tmp }
}

fn amplitude_modifier(modifiers: &[f64], index: i32) -> f64 {
    if modifiers.is_empty() { 1.0 } else { modifiers[index as usize] }
}

fn build_octaves(base_octave: i32, base_amplitude: f64, octave_count: i32, normalize: bool, modifiers: &[f64]) -> Vec<OctaveInfo> {
    let mut frequency = 2.0f64.powi(base_octave);
    let mut amplitude = base_amplitude;
    if normalize {
        amplitude *= 0.5f64.powi(-(octave_count - 1)) / (0.5f64.powi(-octave_count) - 1.0);
    }
    let mut octaves = Vec::new();
    for i in 0..octave_count {
        let modifier = amplitude_modifier(modifiers, i);
        if modifier != 0.0 {
            octaves.push(OctaveInfo { index: base_octave + i, frequency, amplitude: amplitude * modifier });
        }
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    octaves
}

fn normalization_factor(target_amplitude: f64, octaves: &[OctaveInfo]) -> f64 {
    let mut variance = 0.0;
    for octave in octaves {
        let deviation = 0.2702247831245211 * octave.amplitude.abs();
        variance += deviation * deviation;
    }
    let input_deviation = variance.sqrt();
    if input_deviation == 0.0 {
        return 0.0;
    }
    target_amplitude * 0.3333333333333333 / (input_deviation * 2.0f64.sqrt())
}

fn parity_normalization_factor(base_amplitude: f64, octave_count: i32, modifiers: &[f64]) -> f64 {
    let mut min_octave = i32::MAX;
    let mut max_octave = i32::MIN;
    for i in 0..octave_count {
        if amplitude_modifier(modifiers, i) != 0.0 {
            min_octave = min_octave.min(i);
            max_octave = max_octave.max(i);
        }
    }
    let span = max_octave.wrapping_sub(min_octave);
    base_amplitude * 0.5 * 0.3333333333333333 / (0.1 * (1.0 + 1.0 / (span + 1) as f64))
}

impl NormalNoise {
    pub fn create_parity(first_octave: i32, amplitudes: &[f64]) -> NormalNoise {
        let octave_count = amplitudes.len() as i32;
        let octaves = build_octaves(first_octave, 1.0, octave_count, true, amplitudes);
        let target_amplitude = java_double_sum(octaves.iter().map(|o| o.amplitude.abs()));
        let new_normalization_factor = normalization_factor(target_amplitude, &octaves);
        let base_amplitude = if new_normalization_factor == 0.0 {
            1.0
        } else {
            parity_normalization_factor(1.0, octave_count, amplitudes) / new_normalization_factor
        };
        let amplitude_modifiers = if amplitudes.iter().all(|&modifier| modifier == 1.0) { Vec::new() } else { amplitudes.to_vec() };
        NormalNoise::new(NormalNoiseParameters { base_amplitude, base_octave: first_octave, octave_count, normalize: Normalization::Enabled, amplitude_modifiers })
    }

    pub fn new(parameters: NormalNoiseParameters) -> NormalNoise {
        let p = &parameters;
        let octaves = build_octaves(p.base_octave, p.base_amplitude, p.octave_count, p.normalize != Normalization::Disabled, &p.amplitude_modifiers);
        let mut target_amplitude = java_double_sum(octaves.iter().map(|o| o.amplitude.abs()));
        let mut factor = normalization_factor(target_amplitude, &octaves);
        if p.normalize == Normalization::Legacy && factor != 0.0 {
            let parity = parity_normalization_factor(p.base_amplitude, p.octave_count, &p.amplitude_modifiers);
            target_amplitude *= parity / factor;
            factor = parity;
        }
        let range = Interval::of_symmetric((target_amplitude * 0.3333333333333333 * 6.0) as f32);
        NormalNoise { parameters, octaves, normalization_factor: factor, range }
    }

    pub fn range(&self) -> Interval {
        self.range
    }

    pub fn create(&self, random: &mut AlgorithmRandom) -> NoiseStack {
        let first = random.fork_positional();
        let second = random.fork_positional();
        let mut layers = Vec::with_capacity(self.octaves.len() * 2);
        for octave in &self.octaves {
            let seed = format!("octave_{}", octave.index);
            let value_factor = (self.normalization_factor * octave.amplitude) as f32;
            layers.push(Layer { noise: PerlinNoise::new(&mut first.from_hash_of(&seed)), frequency: octave.frequency, amplitude: value_factor });
            layers.push(Layer {
                noise: PerlinNoise::new(&mut second.from_hash_of(&seed)),
                frequency: octave.frequency * 1.0181268882175227,
                amplitude: value_factor,
            });
        }
        NoiseStack::new(layers)
    }

    pub fn create_for_legacy_nether_biome(&self, random: &mut impl RandomSource) -> NoiseStack {
        let p = &self.parameters;
        let amplitudes = if p.amplitude_modifiers.is_empty() { vec![1.0; p.octave_count as usize] } else { p.amplitude_modifiers.clone() };
        let first = legacy_fbm(random, p.base_octave, &amplitudes);
        let second = legacy_fbm(random, p.base_octave, &amplitudes);
        let value_factor = (self.normalization_factor * p.base_amplitude) as f32;
        let mut layers = Vec::new();
        for (stack, frequency) in [(first, 1.0), (second, 1.0181268882175227)] {
            for layer in stack.layers {
                layers.push(Layer { noise: layer.noise, frequency: layer.frequency * frequency, amplitude: layer.amplitude * value_factor });
            }
        }
        NoiseStack::new(layers)
    }
}

fn legacy_fbm(random: &mut impl RandomSource, first_octave: i32, amplitudes: &[f64]) -> NoiseStack {
    let octaves = amplitudes.len() as i32;
    let zero_octave_index = -first_octave;
    let mut levels: Vec<Option<PerlinNoise>> = vec![None; octaves as usize];
    let zero_octave = PerlinNoise::new(random);
    if zero_octave_index >= 0 && zero_octave_index < octaves && amplitudes[zero_octave_index as usize] != 0.0 {
        levels[zero_octave_index as usize] = Some(zero_octave);
    }
    for i in (0..zero_octave_index).rev() {
        if i < octaves && amplitudes[i as usize] != 0.0 {
            levels[i as usize] = Some(PerlinNoise::new(random));
        } else {
            random.consume_count(262);
        }
    }
    assert!(zero_octave_index >= octaves - 1, "Positive octaves are temporarily disabled");
    let mut factor = 2.0f64.powi(-zero_octave_index);
    let mut value_factor = 2.0f64.powi(octaves - 1) / (2.0f64.powi(octaves) - 1.0);
    let mut layers = Vec::new();
    for (i, level) in levels.into_iter().enumerate() {
        if let Some(noise) = level {
            layers.push(Layer { noise, frequency: factor, amplitude: (value_factor * amplitudes[i]) as f32 });
        }
        factor *= 2.0;
        value_factor /= 2.0;
    }
    NoiseStack::new(layers)
}

pub fn blended_fbm(random: &mut impl RandomSource, first_octave: i32, smear_scale_y: f64, value_factor: f64) -> NoiseStack {
    let octaves = -first_octave + 1;
    let mut factor = 1.0;
    let mut value_factor = value_factor / (2.0f64.powi(octaves) - 1.0);
    let mut layers = Vec::new();
    for _ in 0..octaves {
        layers.push(Layer { noise: PerlinNoise::new_smeared(random, smear_scale_y * factor), frequency: factor, amplitude: value_factor as f32 });
        factor /= 2.0;
        value_factor *= 2.0;
    }
    NoiseStack::new(layers)
}

pub fn blended_fbm_range(first_octave: i32, smear_scale_y: f64, value_factor: f64) -> Interval {
    let octaves = -first_octave + 1;
    let mut factor = 1.0;
    let mut value_factor = value_factor / (2.0f64.powi(octaves) - 1.0);
    let mut range = Interval::of_exact(0.0);
    for _ in 0..octaves {
        range = Interval::add(range, Interval::mul(smeared_range(smear_scale_y * factor), Interval::of_exact(value_factor as f32)));
        factor /= 2.0;
        value_factor *= 2.0;
    }
    range
}

pub struct SimplexStack {
    layers: Vec<(SimplexNoise, f64, f32)>,
}

impl SimplexStack {
    pub fn new(layers: Vec<(SimplexNoise, f64, f32)>) -> SimplexStack {
        SimplexStack { layers }
    }

    pub fn get_2d(&self, x: f64, y: f64) -> f32 {
        let mut value = 0.0f32;
        for (noise, frequency, amplitude) in &self.layers {
            value += amplitude * noise.get_2d(x * frequency, y * frequency);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_keeps_small_values() {
        assert_eq!(wrap(123.5), 123.5);
        assert_eq!(wrap(16777216.0), 16777216.0 - 33554432.0);
    }
}
