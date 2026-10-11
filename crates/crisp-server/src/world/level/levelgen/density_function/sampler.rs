use std::sync::Arc;

use crisp_core::mth::{jclamp_f, jmax_f, jmin_f, java_signum_f, lerp3_f, lerp_f};

use super::context::{DensityVolume, SamplerContext};
use super::spline::SplineSampler;
use super::{Axis, DistanceMetric, RoundType, leaky_relu, squeeze};
use crate::world::level::levelgen::synth::{NoiseStack, SimplexNoise};

#[derive(Clone, Copy)]
pub enum GradientKind {
    Clamped { min: i32, max: i32 },
    Repeat { range: i32 },
    MirroredRepeat { range: i32 },
}

#[derive(Clone, Copy)]
pub enum UnaryOp {
    Abs,
    Square,
    Cube,
    Sqrt,
    LeakyReLU(f32),
    Reciprocal,
    Negate,
    Squeeze,
    Log,
    Sign,
}

impl UnaryOp {
    #[inline(always)]
    fn apply(self, v: f32) -> f32 {
        match self {
            UnaryOp::Abs => v.abs(),
            UnaryOp::Square => v * v,
            UnaryOp::Cube => v * v * v,
            UnaryOp::Sqrt => (v as f64).sqrt() as f32,
            UnaryOp::LeakyReLU(factor) => leaky_relu(factor, v),
            UnaryOp::Reciprocal => 1.0 / v,
            UnaryOp::Negate => -v,
            UnaryOp::Squeeze => squeeze(v),
            UnaryOp::Log => (v as f64).ln() as f32,
            UnaryOp::Sign => java_signum_f(v),
        }
    }
}

type B = Box<Sampler>;

pub enum Sampler {
    Constant(f32),
    Beardifier,
    Noise { noise: Arc<NoiseStack>, xz_scale: f64, y_scale: f64 },
    ShiftedXz { shift_x: B, shift_z: B, noise: Arc<NoiseStack>, xz_scale: f64, y_scale: f64 },
    ShiftedXyz { shift_x: B, shift_y: B, shift_z: B, noise: Arc<NoiseStack>, xz_scale: f64, y_scale: f64 },
    ShiftB(Arc<NoiseStack>),
    Gradient { axis: Axis, kind: GradientKind, from: i32, from_value: f32, factor: f32 },
    DistanceToPoint { x: i32, y: i32, z: i32, metric: DistanceMetric },
    EndIslands(Arc<SimplexNoise>),
    Unary(UnaryOp, B),
    RoundInteger(RoundType, B),
    Round(RoundType, B, B),
    Add(B, B),
    ConstAdd(B, f32),
    ConstSub(f32, B),
    Sub(B, B),
    Mul(B, B),
    ConstMul(B, f32),
    Div(B, B),
    ConstDiv(f32, B),
    Min(B, B, f32),
    ConstMin(B, f32),
    Max(B, B, f32),
    ConstMax(B, f32),
    PowConstBase(f64, B),
    PowConstExponent(B, f64),
    Pow(B, B),
    Lerp(B, B, B),
    LerpConstFirst(B, f32, B),
    LerpConstSecond(B, B, f32),
    Clamp(B, f32, f32),
    RangeChoiceConst { input: B, min: f32, max: f32, in_range: f32, out_of_range: f32 },
    RangeChoice { input: B, min: f32, max: f32, in_range: B, out_of_range: B },
    IntervalSelect { input: B, thresholds: Box<[f32]>, samplers: Box<[Sampler]> },
    IntervalSelectSingle { input: B, threshold: f32, below: B, above: B },
    Cache { id: usize, input: Arc<Sampler> },
    BlendDensity(B),
    Interpolated { input: B, cell_xz: i32, cell_y: i32, cell_xz_inv: f32, cell_y_inv: f32 },
    SliceX(B, i32),
    SliceY(B, i32),
    SliceZ(B, i32),
    SliceXz(B, i32, i32),
    FindTopSurface { density: B, upper_bound: B, lower_bound: i32, cell_height: i32 },
    Spline(Box<SplineSampler>),
}

impl Sampler {
    #[inline(always)]
    fn with_right(&self, left: &Sampler, right: &Sampler, ctx: &mut SamplerContext, out: &mut [f32], volume: &DensityVolume, op: impl Fn(f32, f32) -> f32) {
        left.sample_volume(ctx, out, volume);
        let mut right_buffer = ctx.acquire(volume.size());
        right.sample_volume(ctx, &mut right_buffer, volume);
        for (o, r) in out.iter_mut().zip(&right_buffer) {
            *o = op(*o, *r);
        }
        ctx.release(right_buffer);
    }

    pub fn sample_volume(&self, ctx: &mut SamplerContext, out: &mut [f32], volume: &DensityVolume) {
        use Sampler::*;
        match self {
            Constant(v) => out.fill(*v),
            Beardifier => match &ctx.beardifier {
                Some(beardifier) => beardifier.sample_volume(out, volume),
                None => out.fill(0.0),
            },
            Noise { noise, xz_scale, y_scale } => {
                out.fill(0.0);
                noise.add_to_volume(out, volume, *xz_scale, *y_scale, 1.0);
            }
            ShiftedXyz { shift_x, shift_y, shift_z, noise, xz_scale, y_scale } => {
                shift_x.sample_volume(ctx, out, volume);
                let mut shift_y_buffer = ctx.acquire(volume.size());
                shift_y.sample_volume(ctx, &mut shift_y_buffer, volume);
                let mut shift_z_buffer = ctx.acquire(volume.size());
                shift_z.sample_volume(ctx, &mut shift_z_buffer, volume);
                let mut index = 0;
                for z in 0..volume.size_z {
                    let base_z = volume.block_z(z) as f64 * xz_scale;
                    for x in 0..volume.size_x {
                        let base_x = volume.block_x(x) as f64 * xz_scale;
                        for y in 0..volume.size_y {
                            let nx = base_x + out[index] as f64;
                            let ny = volume.block_y(y) as f64 * y_scale + shift_y_buffer[index] as f64;
                            let nz = base_z + shift_z_buffer[index] as f64;
                            out[index] = noise.get(nx, ny, nz);
                            index += 1;
                        }
                    }
                }
                ctx.release(shift_z_buffer);
                ctx.release(shift_y_buffer);
            }
            ShiftedXz { shift_x, shift_z, noise, xz_scale, y_scale } => {
                shift_x.sample_volume(ctx, out, volume);
                let mut shift_z_buffer = ctx.acquire(volume.size());
                shift_z.sample_volume(ctx, &mut shift_z_buffer, volume);
                let mut index = 0;
                for z in 0..volume.size_z {
                    let base_z = volume.block_z(z) as f64 * xz_scale;
                    for x in 0..volume.size_x {
                        let base_x = volume.block_x(x) as f64 * xz_scale;
                        for y in 0..volume.size_y {
                            let nx = base_x + out[index] as f64;
                            let ny = volume.block_y(y) as f64 * y_scale;
                            let nz = base_z + shift_z_buffer[index] as f64;
                            out[index] = noise.get(nx, ny, nz);
                            index += 1;
                        }
                    }
                }
                ctx.release(shift_z_buffer);
            }
            ShiftB(noise) => {
                let transposed = DensityVolume::stepped(
                    volume.size_z,
                    volume.size_x,
                    1,
                    volume.min_z,
                    volume.min_x,
                    0,
                    volume.step_z,
                    volume.step_x,
                    1,
                );
                let mut buffer = ctx.acquire(transposed.size());
                buffer.fill(0.0);
                noise.add_to_volume(&mut buffer, &transposed, 0.25, 0.25, 4.0);
                for z in 0..volume.size_z {
                    for x in 0..volume.size_x {
                        let value = buffer[transposed.index(z, x, 0)];
                        let start = volume.index(x, 0, z);
                        out[start..start + volume.size_y as usize].fill(value);
                    }
                }
                ctx.release(buffer);
            }
            Gradient { axis, .. } => match axis {
                Axis::X => {
                    for x in 0..volume.size_x {
                        let value = self.gradient(volume.block_x(x));
                        for z in 0..volume.size_z {
                            let start = volume.index(x, 0, z);
                            out[start..start + volume.size_y as usize].fill(value);
                        }
                    }
                }
                Axis::Y => {
                    for y in 0..volume.size_y {
                        let value = self.gradient(volume.block_y(y));
                        for z in 0..volume.size_z {
                            for x in 0..volume.size_x {
                                out[volume.index(x, y, z)] = value;
                            }
                        }
                    }
                }
                Axis::Z => {
                    for z in 0..volume.size_z {
                        let value = self.gradient(volume.block_z(z));
                        let start = volume.index(0, 0, z);
                        out[start..start + (volume.size_x * volume.size_y) as usize].fill(value);
                    }
                }
            },
            DistanceToPoint { .. } => sample_volume_naive(self, ctx, out, volume),
            EndIslands(_) => {
                for z in 0..volume.size_z {
                    let block_z = volume.block_z(z);
                    for x in 0..volume.size_x {
                        let value = self.sample_value(ctx, volume.block_x(x), 0, block_z);
                        let start = volume.index(x, 0, z);
                        out[start..start + volume.size_y as usize].fill(value);
                    }
                }
            }
            Unary(op, input) => {
                input.sample_volume(ctx, out, volume);
                let op = *op;
                for v in out.iter_mut() {
                    *v = op.apply(*v);
                }
            }
            RoundInteger(kind, input) => {
                input.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v = kind.apply(*v);
                }
            }
            Round(kind, input, multiple) => self.with_right(input, multiple, ctx, out, volume, |i, m| round_multiple(*kind, i, m)),
            Add(left, right) => self.with_right(left, right, ctx, out, volume, |l, r| l + r),
            ConstAdd(left, right) => {
                left.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v += *right;
                }
            }
            ConstSub(left, right) => {
                right.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v = *left - *v;
                }
            }
            Sub(left, right) => self.with_right(left, right, ctx, out, volume, |l, r| l + -r),
            Mul(left, right) => self.with_right(left, right, ctx, out, volume, |l, r| l * r),
            ConstMul(left, right) => {
                left.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v *= *right;
                }
            }
            Div(left, right) => self.with_right(left, right, ctx, out, volume, |l, r| l / r),
            ConstDiv(left, right) => {
                right.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v = *left / *v;
                }
            }
            Min(left, right, _) => self.with_right(left, right, ctx, out, volume, |l, r| if r < l { r } else { l }),
            ConstMin(left, right) => {
                left.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    if *right < *v {
                        *v = *right;
                    }
                }
            }
            Max(left, right, _) => self.with_right(left, right, ctx, out, volume, |l, r| if r > l { r } else { l }),
            ConstMax(left, right) => {
                left.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    if *right > *v {
                        *v = *right;
                    }
                }
            }
            PowConstBase(base, exponent) => {
                exponent.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v = base.powf(*v as f64) as f32;
                }
            }
            PowConstExponent(base, exponent) => {
                base.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v = (*v as f64).powf(*exponent) as f32;
                }
            }
            Pow(base, exponent) => self.with_right(base, exponent, ctx, out, volume, |b, e| (b as f64).powf(e as f64) as f32),
            Lerp(alpha, first, second) => {
                alpha.sample_volume(ctx, out, volume);
                let mut first_buffer = ctx.acquire(volume.size());
                first.sample_volume(ctx, &mut first_buffer, volume);
                let mut second_buffer = ctx.acquire(volume.size());
                second.sample_volume(ctx, &mut second_buffer, volume);
                for i in 0..out.len() {
                    out[i] = lerp_value(out[i], first_buffer[i], second_buffer[i]);
                }
                ctx.release(second_buffer);
                ctx.release(first_buffer);
            }
            LerpConstFirst(alpha, first, second) => {
                alpha.sample_volume(ctx, out, volume);
                let mut second_buffer = ctx.acquire(volume.size());
                second.sample_volume(ctx, &mut second_buffer, volume);
                for i in 0..out.len() {
                    out[i] = lerp_value(out[i], *first, second_buffer[i]);
                }
                ctx.release(second_buffer);
            }
            LerpConstSecond(alpha, first, second) => {
                alpha.sample_volume(ctx, out, volume);
                let mut first_buffer = ctx.acquire(volume.size());
                first.sample_volume(ctx, &mut first_buffer, volume);
                for i in 0..out.len() {
                    out[i] = lerp_value(out[i], first_buffer[i], *second);
                }
                ctx.release(first_buffer);
            }
            Clamp(input, min, max) => {
                input.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v = jclamp_f(*v, *min, *max);
                }
            }
            RangeChoiceConst { input, min, max, in_range, out_of_range } => {
                input.sample_volume(ctx, out, volume);
                for v in out.iter_mut() {
                    *v = if *v >= *min && *v < *max { *in_range } else { *out_of_range };
                }
            }
            RangeChoice { input, min, max, in_range, out_of_range } => {
                in_range.sample_volume(ctx, out, volume);
                let mut input_buffer = ctx.acquire(volume.size());
                input.sample_volume(ctx, &mut input_buffer, volume);
                let mut out_buffer = ctx.acquire(volume.size());
                out_of_range.sample_volume(ctx, &mut out_buffer, volume);
                for i in 0..out.len() {
                    let v = input_buffer[i];
                    if !(v >= *min) || !(v < *max) {
                        out[i] = out_buffer[i];
                    }
                }
                ctx.release(out_buffer);
                ctx.release(input_buffer);
            }
            IntervalSelect { input, thresholds, samplers } => {
                input.sample_volume(ctx, out, volume);
                let mut buffers = Vec::with_capacity(samplers.len());
                for sampler in samplers.iter() {
                    let mut buffer = ctx.acquire(volume.size());
                    sampler.sample_volume(ctx, &mut buffer, volume);
                    buffers.push(buffer);
                }
                for i in 0..out.len() {
                    let index = select_interval(thresholds, out[i]);
                    out[i] = buffers[index][i];
                }
                for buffer in buffers {
                    ctx.release(buffer);
                }
            }
            IntervalSelectSingle { input, threshold, below, above } => {
                input.sample_volume(ctx, out, volume);
                let mut below_buffer = ctx.acquire(volume.size());
                below.sample_volume(ctx, &mut below_buffer, volume);
                let mut above_buffer = ctx.acquire(volume.size());
                above.sample_volume(ctx, &mut above_buffer, volume);
                for i in 0..out.len() {
                    out[i] = if out[i] < *threshold { below_buffer[i] } else { above_buffer[i] };
                }
                ctx.release(above_buffer);
                ctx.release(below_buffer);
            }
            Cache { id, input } => ctx.sample_volume_cached(*id, input, out, volume),
            BlendDensity(input) => input.sample_volume(ctx, out, volume),
            Interpolated { input, cell_xz, cell_y, .. } => {
                let (xz, y) = (*cell_xz, *cell_y);
                if (volume.step_x == xz || volume.size_x == 1)
                    && (volume.step_y == y || volume.size_y == 1)
                    && (volume.step_z == xz || volume.size_z == 1)
                    && volume.min_x.rem_euclid(xz) == 0
                    && volume.min_y.rem_euclid(y) == 0
                    && volume.min_z.rem_euclid(xz) == 0
                {
                    input.sample_volume(ctx, out, volume);
                } else if volume.step_x == 1 && volume.step_y == 1 && volume.step_z == 1 {
                    self.interpolate_block_step(ctx, out, volume);
                } else {
                    let block_volume = DensityVolume::new(
                        volume.size_x * volume.step_x,
                        volume.size_y * volume.step_y,
                        volume.size_z * volume.step_z,
                        volume.min_x,
                        volume.min_y,
                        volume.min_z,
                    );
                    let mut block_buffer = ctx.acquire(block_volume.size());
                    self.interpolate_block_step(ctx, &mut block_buffer, &block_volume);
                    for z in 0..volume.size_z {
                        for x in 0..volume.size_x {
                            for y in 0..volume.size_y {
                                out[volume.index(x, y, z)] =
                                    block_buffer[block_volume.index(x * volume.step_x, y * volume.step_y, z * volume.step_z)];
                            }
                        }
                    }
                    ctx.release(block_buffer);
                }
            }
            SliceX(input, sx) => {
                if volume.size_x == 1 && volume.min_x == *sx {
                    input.sample_volume(ctx, out, volume);
                } else {
                    let iv = DensityVolume::stepped(1, volume.size_y, volume.size_z, *sx, volume.min_y, volume.min_z, volume.step_x, volume.step_y, volume.step_z);
                    let mut buffer = ctx.acquire(iv.size());
                    input.sample_volume(ctx, &mut buffer, &iv);
                    let mut index = 0;
                    for z in 0..volume.size_z {
                        for _ in 0..volume.size_x {
                            for y in 0..volume.size_y {
                                out[index] = buffer[iv.index(0, y, z)];
                                index += 1;
                            }
                        }
                    }
                    ctx.release(buffer);
                }
            }
            SliceXz(input, sx, sz) => {
                if volume.size_x == 1 && volume.size_z == 1 && volume.min_x == *sx && volume.min_z == *sz {
                    input.sample_volume(ctx, out, volume);
                } else {
                    let iv = DensityVolume::stepped(1, volume.size_y, 1, *sx, volume.min_y, *sz, volume.step_x, volume.step_y, volume.step_z);
                    let mut buffer = ctx.acquire(iv.size());
                    input.sample_volume(ctx, &mut buffer, &iv);
                    for y in 0..volume.size_y {
                        let value = buffer[iv.index(0, y, 0)];
                        let mut index = volume.index(0, y, 0);
                        for _ in 0..(volume.size_x * volume.size_z) {
                            out[index] = value;
                            index += volume.size_y as usize;
                        }
                    }
                    ctx.release(buffer);
                }
            }
            SliceY(input, sy) => {
                if volume.size_y == 1 && volume.min_y == *sy {
                    input.sample_volume(ctx, out, volume);
                } else {
                    let iv = DensityVolume::stepped(volume.size_x, 1, volume.size_z, volume.min_x, *sy, volume.min_z, volume.step_x, volume.step_y, volume.step_z);
                    let mut buffer = ctx.acquire(iv.size());
                    input.sample_volume(ctx, &mut buffer, &iv);
                    for z in 0..volume.size_z {
                        for x in 0..volume.size_x {
                            let value = buffer[iv.index(x, 0, z)];
                            let start = volume.index(x, 0, z);
                            out[start..start + volume.size_y as usize].fill(value);
                        }
                    }
                    ctx.release(buffer);
                }
            }
            SliceZ(input, sz) => {
                if volume.size_z == 1 && volume.min_z == *sz {
                    input.sample_volume(ctx, out, volume);
                } else {
                    let iv = DensityVolume::stepped(volume.size_x, volume.size_y, 1, volume.min_x, volume.min_y, *sz, volume.step_x, volume.step_y, volume.step_z);
                    let mut buffer = ctx.acquire(iv.size());
                    input.sample_volume(ctx, &mut buffer, &iv);
                    let mut index = 0;
                    for _ in 0..volume.size_z {
                        for x in 0..volume.size_x {
                            for y in 0..volume.size_y {
                                out[index] = buffer[iv.index(x, y, 0)];
                                index += 1;
                            }
                        }
                    }
                    ctx.release(buffer);
                }
            }
            FindTopSurface { upper_bound, .. } => {
                assert!(volume.size_y == 1, "Cannot sample with sizeY={}", volume.size_y);
                upper_bound.sample_volume(ctx, out, volume);
                let mut index = 0;
                for z in 0..volume.size_z {
                    let block_z = volume.block_z(z);
                    for x in 0..volume.size_x {
                        let block_x = volume.block_x(x);
                        out[index] = self.find_surface(ctx, block_x, block_z, out[index]) as f32;
                        index += 1;
                    }
                }
            }
            Spline(spline) => spline.sample_volume(ctx, out, volume),
        }
    }

    pub fn sample_value(&self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32) -> f32 {
        use Sampler::*;
        match self {
            Constant(v) => *v,
            Beardifier => ctx.beardifier.as_ref().map_or(0.0, |beardifier| beardifier.sample_value(x, y, z)),
            Noise { noise, xz_scale, y_scale } => noise.get(x as f64 * xz_scale, y as f64 * y_scale, z as f64 * xz_scale),
            ShiftedXyz { shift_x, shift_y, shift_z, noise, xz_scale, y_scale } => {
                let nx = x as f64 * xz_scale + shift_x.sample_value(ctx, x, y, z) as f64;
                let ny = y as f64 * y_scale + shift_y.sample_value(ctx, x, y, z) as f64;
                let nz = z as f64 * xz_scale + shift_z.sample_value(ctx, x, y, z) as f64;
                noise.get(nx, ny, nz)
            }
            ShiftedXz { shift_x, shift_z, noise, xz_scale, y_scale } => {
                let nx = x as f64 * xz_scale + shift_x.sample_value(ctx, x, y, z) as f64;
                let ny = y as f64 * y_scale;
                let nz = z as f64 * xz_scale + shift_z.sample_value(ctx, x, y, z) as f64;
                noise.get(nx, ny, nz)
            }
            ShiftB(noise) => noise.get(z as f64 * 0.25, x as f64 * 0.25, 0.0) * 4.0,
            Gradient { axis, .. } => self.gradient(axis.choose(x, y, z)),
            DistanceToPoint { x: px, y: py, z: pz, metric } => metric.compute((px - x) as f32, (py - y) as f32, (pz - z) as f32),
            EndIslands(noise) => (end_island_height(noise, x / 8, z / 8) - 8.0) / 128.0,
            Unary(op, input) => op.apply(input.sample_value(ctx, x, y, z)),
            RoundInteger(kind, input) => kind.apply(input.sample_value(ctx, x, y, z)),
            Round(kind, input, multiple) => {
                let i = input.sample_value(ctx, x, y, z);
                let m = multiple.sample_value(ctx, x, y, z);
                round_multiple(*kind, i, m)
            }
            Add(left, right) => left.sample_value(ctx, x, y, z) + right.sample_value(ctx, x, y, z),
            ConstAdd(left, right) => left.sample_value(ctx, x, y, z) + right,
            ConstSub(left, right) => left - right.sample_value(ctx, x, y, z),
            Sub(left, right) => left.sample_value(ctx, x, y, z) - right.sample_value(ctx, x, y, z),
            Mul(left, right) => {
                let l = left.sample_value(ctx, x, y, z);
                if l == 0.0 { 0.0 } else { l * right.sample_value(ctx, x, y, z) }
            }
            ConstMul(left, right) => left.sample_value(ctx, x, y, z) * right,
            Div(left, right) => {
                let l = left.sample_value(ctx, x, y, z);
                if l == 0.0 { 0.0 } else { l / right.sample_value(ctx, x, y, z) }
            }
            ConstDiv(left, right) => left / right.sample_value(ctx, x, y, z),
            Min(left, right, right_min) => {
                let l = left.sample_value(ctx, x, y, z);
                if l <= *right_min { l } else { jmin_f(l, right.sample_value(ctx, x, y, z)) }
            }
            ConstMin(left, right) => jmin_f(left.sample_value(ctx, x, y, z), *right),
            Max(left, right, right_max) => {
                let l = left.sample_value(ctx, x, y, z);
                if l >= *right_max { l } else { jmax_f(l, right.sample_value(ctx, x, y, z)) }
            }
            ConstMax(left, right) => jmax_f(left.sample_value(ctx, x, y, z), *right),
            PowConstBase(base, exponent) => base.powf(exponent.sample_value(ctx, x, y, z) as f64) as f32,
            PowConstExponent(base, exponent) => (base.sample_value(ctx, x, y, z) as f64).powf(*exponent) as f32,
            Pow(base, exponent) => {
                let b = base.sample_value(ctx, x, y, z);
                (b as f64).powf(exponent.sample_value(ctx, x, y, z) as f64) as f32
            }
            Lerp(alpha, first, second) => {
                let a = alpha.sample_value(ctx, x, y, z);
                if a == 0.0 {
                    first.sample_value(ctx, x, y, z)
                } else if a == 1.0 {
                    second.sample_value(ctx, x, y, z)
                } else {
                    let f = first.sample_value(ctx, x, y, z);
                    lerp_f(a, f, second.sample_value(ctx, x, y, z))
                }
            }
            LerpConstFirst(alpha, first, second) => {
                let a = alpha.sample_value(ctx, x, y, z);
                if a == 0.0 {
                    *first
                } else if a == 1.0 {
                    second.sample_value(ctx, x, y, z)
                } else {
                    lerp_f(a, *first, second.sample_value(ctx, x, y, z))
                }
            }
            LerpConstSecond(alpha, first, second) => {
                let a = alpha.sample_value(ctx, x, y, z);
                if a == 0.0 {
                    first.sample_value(ctx, x, y, z)
                } else if a == 1.0 {
                    *second
                } else {
                    lerp_f(a, first.sample_value(ctx, x, y, z), *second)
                }
            }
            Clamp(input, min, max) => jclamp_f(input.sample_value(ctx, x, y, z), *min, *max),
            RangeChoiceConst { input, min, max, in_range, out_of_range } => {
                let v = input.sample_value(ctx, x, y, z);
                if v >= *min && v < *max { *in_range } else { *out_of_range }
            }
            RangeChoice { input, min, max, in_range, out_of_range } => {
                let v = input.sample_value(ctx, x, y, z);
                if v >= *min && v < *max { in_range.sample_value(ctx, x, y, z) } else { out_of_range.sample_value(ctx, x, y, z) }
            }
            IntervalSelect { input, thresholds, samplers } => {
                let v = input.sample_value(ctx, x, y, z);
                samplers[select_interval(thresholds, v)].sample_value(ctx, x, y, z)
            }
            IntervalSelectSingle { input, threshold, below, above } => {
                let v = input.sample_value(ctx, x, y, z);
                if v < *threshold { below.sample_value(ctx, x, y, z) } else { above.sample_value(ctx, x, y, z) }
            }
            Cache { id, input } => ctx.sample_value_cached(*id, input, x, y, z),
            BlendDensity(input) => input.sample_value(ctx, x, y, z),
            Interpolated { input, cell_xz, cell_y, .. } => {
                let (xz, cy) = (*cell_xz, *cell_y);
                let (xi, yi, zi) = (x.rem_euclid(xz), y.rem_euclid(cy), z.rem_euclid(xz));
                if xi == 0 && yi == 0 && zi == 0 {
                    return input.sample_value(ctx, x, y, z);
                }
                let volume = DensityVolume::stepped(2, 2, 2, x - xi, y - yi, z - zi, xz, cy, xz);
                let mut buffer = ctx.acquire(volume.size());
                input.sample_volume(ctx, &mut buffer, &volume);
                let value = lerp3_f(
                    xi as f32 / xz as f32,
                    yi as f32 / cy as f32,
                    zi as f32 / xz as f32,
                    buffer[volume.index(0, 0, 0)],
                    buffer[volume.index(1, 0, 0)],
                    buffer[volume.index(0, 1, 0)],
                    buffer[volume.index(1, 1, 0)],
                    buffer[volume.index(0, 0, 1)],
                    buffer[volume.index(1, 0, 1)],
                    buffer[volume.index(0, 1, 1)],
                    buffer[volume.index(1, 1, 1)],
                );
                ctx.release(buffer);
                value
            }
            SliceX(input, sx) => input.sample_value(ctx, *sx, y, z),
            SliceY(input, sy) => input.sample_value(ctx, x, *sy, z),
            SliceZ(input, sz) => input.sample_value(ctx, x, y, *sz),
            SliceXz(input, sx, sz) => input.sample_value(ctx, *sx, y, *sz),
            FindTopSurface { upper_bound, .. } => {
                let upper = upper_bound.sample_value(ctx, x, y, z);
                self.find_surface(ctx, x, z, upper) as f32
            }
            Spline(spline) => spline.sample_value(ctx, x, y, z),
        }
    }

    #[inline(always)]
    fn gradient(&self, coordinate: i32) -> f32 {
        let Sampler::Gradient { kind, from, from_value, factor, .. } = self else { unreachable!() };
        match *kind {
            GradientKind::Clamped { min, max } => from_value + (coordinate.max(min).min(max) - from) as f32 * factor,
            GradientKind::Repeat { range } => from_value + floor_mod(coordinate - from, range) as f32 * factor,
            GradientKind::MirroredRepeat { range } => {
                let relative = coordinate - from;
                let tile = floor_div(relative, range);
                let local = relative - tile * range;
                if tile & 1 == 0 { from_value + local as f32 * factor } else { from_value + (range - local) as f32 * factor }
            }
        }
    }

    fn find_surface(&self, ctx: &mut SamplerContext, x: i32, z: i32, upper_bound: f32) -> i32 {
        let Sampler::FindTopSurface { density, lower_bound, cell_height, .. } = self else { unreachable!() };
        let top = (upper_bound / *cell_height as f32).floor() as i32 * cell_height;
        if top <= *lower_bound {
            return *lower_bound;
        }
        let mut probe = top;
        while probe >= *lower_bound {
            if density.sample_value(ctx, x, probe, z) > 0.0 {
                return probe;
            }
            probe -= cell_height;
        }
        *lower_bound
    }

    fn interpolate_block_step(&self, ctx: &mut SamplerContext, out: &mut [f32], volume: &DensityVolume) {
        let Sampler::Interpolated { input, cell_xz, cell_y, cell_xz_inv, cell_y_inv } = self else { unreachable!() };
        let (xz, cy) = (*cell_xz, *cell_y);
        let min_cell_x = volume.min_x.div_euclid(xz);
        let min_cell_y = volume.min_y.div_euclid(cy);
        let min_cell_z = volume.min_z.div_euclid(xz);
        let cell_count_x = volume.max_x().div_euclid(xz) - min_cell_x + 1;
        let cell_count_y = volume.max_y().div_euclid(cy) - min_cell_y + 1;
        let cell_count_z = volume.max_z().div_euclid(xz) - min_cell_z + 1;
        let cells = DensityVolume::stepped(
            if volume.max_x().rem_euclid(xz) == 0 { cell_count_x } else { cell_count_x + 1 },
            if volume.max_y().rem_euclid(cy) == 0 { cell_count_y } else { cell_count_y + 1 },
            if volume.max_z().rem_euclid(xz) == 0 { cell_count_z } else { cell_count_z + 1 },
            min_cell_x * xz,
            min_cell_y * cy,
            min_cell_z * xz,
            xz,
            cy,
            xz,
        );
        let mut corners = ctx.acquire(cells.size());
        input.sample_volume(ctx, &mut corners, &cells);
        for cell_z in 0..cell_count_z {
            let next_z = (cell_z + 1).min(cells.size_z - 1);
            for cell_x in 0..cell_count_x {
                let next_x = (cell_x + 1).min(cells.size_x - 1);
                let mut v000 = corners[cells.index(cell_x, 0, cell_z)];
                let mut v100 = corners[cells.index(next_x, 0, cell_z)];
                let mut v001 = corners[cells.index(cell_x, 0, next_z)];
                let mut v101 = corners[cells.index(next_x, 0, next_z)];
                for cell_y_index in 0..cell_count_y {
                    let next_y = (cell_y_index + 1).min(cells.size_y - 1);
                    let v010 = corners[cells.index(cell_x, next_y, cell_z)];
                    let v110 = corners[cells.index(next_x, next_y, cell_z)];
                    let v011 = corners[cells.index(cell_x, next_y, next_z)];
                    let v111 = corners[cells.index(next_x, next_y, next_z)];
                    let output_x = cells.block_x(cell_x) - volume.min_x;
                    let output_y = cells.block_y(cell_y_index) - volume.min_y;
                    let output_z = cells.block_z(cell_z) - volume.min_z;
                    let x0 = 0.max(-output_x);
                    let y0 = 0.max(-output_y);
                    let z0 = 0.max(-output_z);
                    let x1 = xz.min(volume.size_x - output_x) - 1;
                    let y1 = cy.min(volume.size_y - output_y) - 1;
                    let z1 = xz.min(volume.size_z - output_z) - 1;
                    for z in z0..=z1 {
                        let alpha_z = z as f32 * cell_xz_inv;
                        let v00 = lerp_f(alpha_z, v000, v001);
                        let v01 = lerp_f(alpha_z, v010, v011);
                        let v10 = lerp_f(alpha_z, v100, v101);
                        let v11 = lerp_f(alpha_z, v110, v111);
                        for x in x0..=x1 {
                            let alpha_x = x as f32 * cell_xz_inv;
                            let v0 = lerp_f(alpha_x, v00, v10);
                            let v1 = lerp_f(alpha_x, v01, v11);
                            let step = (v1 - v0) * cell_y_inv;
                            let mut value = v0 + step * y0 as f32;
                            let mut index = volume.index(output_x + x, output_y + y0, output_z + z);
                            for _ in y0..=y1 {
                                out[index] = value;
                                index += 1;
                                value += step;
                            }
                        }
                    }
                    v000 = v010;
                    v100 = v110;
                    v001 = v011;
                    v101 = v111;
                }
            }
        }
        ctx.release(corners);
    }
}

fn floor_div(x: i32, y: i32) -> i32 {
    let q = x / y;
    if (x % y != 0) && ((x ^ y) < 0) { q - 1 } else { q }
}

fn floor_mod(x: i32, y: i32) -> i32 {
    x - floor_div(x, y) * y
}

fn sample_volume_naive(sampler: &Sampler, ctx: &mut SamplerContext, out: &mut [f32], volume: &DensityVolume) {
    let mut index = 0;
    for z in 0..volume.size_z {
        let block_z = volume.block_z(z);
        for x in 0..volume.size_x {
            let block_x = volume.block_x(x);
            for y in 0..volume.size_y {
                out[index] = sampler.sample_value(ctx, block_x, volume.block_y(y), block_z);
                index += 1;
            }
        }
    }
}

#[inline(always)]
fn lerp_value(alpha: f32, first: f32, second: f32) -> f32 {
    if alpha == 0.0 {
        first
    } else if alpha == 1.0 {
        second
    } else {
        lerp_f(alpha, first, second)
    }
}

fn round_multiple(kind: RoundType, input: f32, multiple: f32) -> f32 {
    if multiple == 0.0 { input } else { kind.apply(input / multiple) * multiple }
}

fn select_interval(thresholds: &[f32], input: f32) -> usize {
    thresholds.iter().position(|&t| input < t).unwrap_or(thresholds.len())
}

fn end_island_height(noise: &SimplexNoise, section_x: i32, section_z: i32) -> f32 {
    let chunk_x = section_x / 2;
    let chunk_z = section_z / 2;
    let sub_x = section_x % 2;
    let sub_z = section_z % 2;
    let mut doffs = -100.0f32;
    for xo in -12..=12 {
        for zo in -12..=12 {
            let total_x = (chunk_x + xo) as i64;
            let total_z = (chunk_z + zo) as i64;
            if total_x * total_x + total_z * total_z > 4096 && noise.get_2d(total_x as f64, total_z as f64) < -0.9 {
                let island_size = ((total_x as f32).abs() * 3439.0 + (total_z as f32).abs() * 147.0) % 13.0 + 9.0;
                let xd = (sub_x - xo * 2) as f32;
                let zd = (sub_z - zo * 2) as f32;
                let mut new_doffs = 100.0 - ((xd * xd + zd * zd) as f64).sqrt() as f32 * island_size;
                new_doffs = jclamp_f(new_doffs, -100.0, 80.0);
                doffs = jmax_f(doffs, new_doffs);
            }
        }
    }
    doffs
}
