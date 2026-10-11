use std::collections::HashMap;
use std::sync::Arc;

use super::sampler::{GradientKind, Sampler, UnaryOp};
use super::spline::SplineSampler;
use super::{AXIS_X, AXIS_Y, AXIS_Z, Axis, BinaryType, Df, DensityFunction, NoiseHolder, PreparedCache, SimpleType, TilingMode, UnaryType};
use crate::world::level::levelgen::AlgorithmRandom;
use crate::world::level::levelgen::synth::{self, NoiseStack, SimplexNoise};

pub trait CompileContext {
    fn noise(&self, holder: &NoiseHolder) -> Arc<NoiseStack>;
    fn random(&self, name: &str) -> AlgorithmRandom;
    fn end_island_random(&self) -> AlgorithmRandom;
}

#[derive(Default)]
pub struct DensityFunctionCompiler {
    prepared: HashMap<Df, Df>,
    next_cache_id: usize,
    samplers: HashMap<Df, Arc<Sampler>>,
}

impl DensityFunctionCompiler {
    pub fn get_sampler(&mut self, function: &Df, ctx: &dyn CompileContext) -> Arc<Sampler> {
        if let Some(sampler) = self.samplers.get(function) {
            return sampler.clone();
        }
        let optimized = self.optimize(function, ctx);
        let sampler = Arc::new(compile(&optimized, ctx));
        self.samplers.insert(function.clone(), sampler.clone());
        sampler
    }

    fn optimize(&mut self, function: &Df, ctx: &dyn CompileContext) -> Df {
        let inlined = self.inline_and_prepare(function, ctx);
        slice_uniform_axes(super::ALL_AXES, &inlined)
    }

    fn inline_and_prepare(&mut self, function: &Df, ctx: &dyn CompileContext) -> Df {
        let function = match &**function {
            DensityFunction::Reference(r) => r.value.clone(),
            _ => function.clone(),
        };
        if let DensityFunction::Cache(input) = &*function {
            return self.reuse_or_prepare_cache(input, ctx);
        }
        function.rewrite_children(&mut |child| self.inline_and_prepare(child, ctx))
    }

    fn reuse_or_prepare_cache(&mut self, input: &Df, ctx: &dyn CompileContext) -> Df {
        if let Some(prepared) = self.prepared.get(input) {
            return prepared.clone();
        }
        let id = self.next_cache_id;
        self.next_cache_id += 1;
        let optimized = self.optimize(input, ctx);
        let sampler = Arc::new(compile(&optimized, ctx));
        let prepared = Arc::new(DensityFunction::Prepared(Arc::new(PreparedCache {
            id,
            range: optimized.range(),
            domain_axes: optimized.domain_axes(),
            input: sampler,
        })));
        self.prepared.insert(input.clone(), prepared.clone());
        prepared
    }
}

fn slice_uniform_axes(parent_axes: u8, function: &Df) -> Df {
    if matches!(**function, DensityFunction::Constant(_) | DensityFunction::Gradient { .. }) {
        return function.clone();
    }
    let axes = function.domain_axes();
    if parent_axes == axes {
        return function.rewrite_children(&mut |child| slice_uniform_axes(parent_axes, child));
    }
    let mut new = function.rewrite_children(&mut |child| slice_uniform_axes(axes, child));
    let removed = parent_axes & !axes;
    let mut existing = 0;
    let mut inner = &new;
    while let DensityFunction::Slice(axis, _, input) = &**inner {
        existing |= axis.bit();
        inner = input;
    }
    let filtered = removed & !existing;
    if filtered & AXIS_X != 0 {
        new = Arc::new(DensityFunction::Slice(Axis::X, 0, new));
    }
    if filtered & AXIS_Z != 0 {
        new = Arc::new(DensityFunction::Slice(Axis::Z, 0, new));
    }
    if filtered & AXIS_Y != 0 {
        new = Arc::new(DensityFunction::Slice(Axis::Y, 0, new));
    }
    new
}

fn boxed(function: &Df, ctx: &dyn CompileContext) -> Box<Sampler> {
    Box::new(compile(function, ctx))
}

pub fn compile(function: &Df, ctx: &dyn CompileContext) -> Sampler {
    use DensityFunction as F;
    match &**function {
        F::Constant(v) => Sampler::Constant(v.0),
        F::Reference(r) => compile(&r.value, ctx),
        F::Noise { noise, xz_scale, y_scale, shift_x, shift_y, shift_z } => {
            let noise = ctx.noise(noise);
            let (xz_scale, y_scale) = (xz_scale.0, y_scale.0);
            if shift_x.is_zero() && shift_y.is_zero() && shift_z.is_zero() {
                return Sampler::Noise { noise, xz_scale, y_scale };
            }
            let shift_x = boxed(shift_x, ctx);
            let shift_z_sampler = boxed(shift_z, ctx);
            if shift_y.is_zero() {
                return Sampler::ShiftedXz { shift_x, shift_z: shift_z_sampler, noise, xz_scale, y_scale };
            }
            Sampler::ShiftedXyz { shift_x, shift_y: boxed(shift_y, ctx), shift_z: shift_z_sampler, noise, xz_scale, y_scale }
        }
        F::Shift(noise) => Sampler::ConstMul(Box::new(Sampler::Noise { noise: ctx.noise(noise), xz_scale: 0.25, y_scale: 0.25 }), 4.0),
        F::ShiftA(noise) => Sampler::ConstMul(Box::new(Sampler::Noise { noise: ctx.noise(noise), xz_scale: 0.25, y_scale: 0.0 }), 4.0),
        F::ShiftB(noise) => Sampler::ShiftB(ctx.noise(noise)),
        F::Gradient { axis, tiling, from, to, from_value, to_value } => {
            let range = to - from;
            let factor = (to_value.0 - from_value.0) / range as f32;
            let kind = match tiling {
                TilingMode::ClampToEdge => GradientKind::Clamped { min: (*from).min(*to), max: (*from).max(*to) },
                TilingMode::Repeat => GradientKind::Repeat { range },
                TilingMode::MirroredRepeat => GradientKind::MirroredRepeat { range },
            };
            Sampler::Gradient { axis: *axis, kind, from: *from, from_value: from_value.0, factor }
        }
        F::DistanceToPoint { x, y, z, metric } => Sampler::DistanceToPoint { x: *x, y: *y, z: *z, metric: *metric },
        F::EndIslands => {
            let mut random = ctx.end_island_random();
            crisp_core::RandomSource::consume_count(&mut random, 17292);
            Sampler::EndIslands(Arc::new(SimplexNoise::new(&mut random, true)))
        }
        F::Simple(SimpleType::BlendAlpha) => Sampler::Constant(1.0),
        F::Simple(SimpleType::BlendOffset) => Sampler::Constant(0.0),
        F::Simple(SimpleType::Beardifier) => Sampler::Beardifier,
        F::Unary(kind, input) => {
            let op = match kind {
                UnaryType::Abs => UnaryOp::Abs,
                UnaryType::Square => UnaryOp::Square,
                UnaryType::Cube => UnaryOp::Cube,
                UnaryType::Sqrt => UnaryOp::Sqrt,
                UnaryType::HalfNegative => UnaryOp::LeakyReLU(0.5),
                UnaryType::QuarterNegative => UnaryOp::LeakyReLU(0.25),
                UnaryType::Reciprocal => UnaryOp::Reciprocal,
                UnaryType::Negate => UnaryOp::Negate,
                UnaryType::Squeeze => UnaryOp::Squeeze,
                UnaryType::Log => UnaryOp::Log,
                UnaryType::Sign => UnaryOp::Sign,
            };
            Sampler::Unary(op, boxed(input, ctx))
        }
        F::Round(kind, input, multiple) => {
            let input = boxed(input, ctx);
            if multiple.as_constant() == Some(1.0) {
                Sampler::RoundInteger(*kind, input)
            } else {
                Sampler::Round(*kind, input, boxed(multiple, ctx))
            }
        }
        F::Binary(kind, left_fn, right_fn) => {
            let left = boxed(left_fn, ctx);
            let right = boxed(right_fn, ctx);
            let (lc, rc) = (left_fn.as_constant(), right_fn.as_constant());
            match kind {
                BinaryType::Add => match (lc, rc) {
                    (Some(l), _) => Sampler::ConstAdd(right, l),
                    (_, Some(r)) => Sampler::ConstAdd(left, r),
                    _ => Sampler::Add(left, right),
                },
                BinaryType::Sub => match (lc, rc) {
                    (Some(l), _) => Sampler::ConstSub(l, right),
                    (_, Some(r)) => Sampler::ConstAdd(left, -r),
                    _ => Sampler::Sub(left, right),
                },
                BinaryType::Mul => match (lc, rc) {
                    (Some(l), _) => Sampler::ConstMul(right, l),
                    (_, Some(r)) => Sampler::ConstMul(left, r),
                    _ => Sampler::Mul(left, right),
                },
                BinaryType::Div => match (lc, rc) {
                    (Some(l), _) => Sampler::ConstDiv(l, right),
                    (_, Some(r)) => Sampler::ConstMul(left, 1.0 / r),
                    _ => Sampler::Div(left, right),
                },
                BinaryType::Min => {
                    let (lr, rr) = (left_fn.range(), right_fn.range());
                    if lr.max() < rr.min() {
                        *left
                    } else if rr.max() < lr.min() {
                        *right
                    } else {
                        match (lc, rc) {
                            (Some(l), _) => Sampler::ConstMin(right, l),
                            (_, Some(r)) => Sampler::ConstMin(left, r),
                            _ => Sampler::Min(left, right, rr.min()),
                        }
                    }
                }
                BinaryType::Max => {
                    let (lr, rr) = (left_fn.range(), right_fn.range());
                    if lr.min() > rr.max() {
                        *left
                    } else if rr.min() > lr.max() {
                        *right
                    } else {
                        match (lc, rc) {
                            (Some(l), _) => Sampler::ConstMax(right, l),
                            (_, Some(r)) => Sampler::ConstMax(left, r),
                            _ => Sampler::Max(left, right, rr.max()),
                        }
                    }
                }
            }
        }
        F::Pow(base_fn, exponent_fn) => {
            let base = boxed(base_fn, ctx);
            let exponent = boxed(exponent_fn, ctx);
            if let Some(b) = base_fn.as_constant() {
                Sampler::PowConstBase(b as f64, exponent)
            } else if let Some(e) = exponent_fn.as_constant() {
                let special = match e.abs() {
                    0.5 => Sampler::Unary(UnaryOp::Sqrt, base),
                    1.0 => *base,
                    2.0 => Sampler::Unary(UnaryOp::Square, base),
                    3.0 => Sampler::Unary(UnaryOp::Cube, base),
                    _ => return Sampler::PowConstExponent(base, e as f64),
                };
                if e >= 0.0 { special } else { Sampler::Unary(UnaryOp::Reciprocal, Box::new(special)) }
            } else {
                Sampler::Pow(base, exponent)
            }
        }
        F::Spline(spline) => Sampler::Spline(Box::new(SplineSampler::new(spline, &mut |f| compile(f, ctx)))),
        F::Lerp(alpha, first_fn, second_fn) => {
            let alpha = boxed(alpha, ctx);
            let first = boxed(first_fn, ctx);
            let second = boxed(second_fn, ctx);
            if let Some(f) = first_fn.as_constant() {
                Sampler::LerpConstFirst(alpha, f, second)
            } else if let Some(s) = second_fn.as_constant() {
                Sampler::LerpConstSecond(alpha, first, s)
            } else {
                Sampler::Lerp(alpha, first, second)
            }
        }
        F::Clamp(input, min, max) => Sampler::Clamp(boxed(input, ctx), min.0, max.0),
        F::RangeChoice { input, min, max, in_range, out_of_range } => {
            let input = boxed(input, ctx);
            match (in_range.as_constant(), out_of_range.as_constant()) {
                (Some(i), Some(o)) => Sampler::RangeChoiceConst { input, min: min.0, max: max.0, in_range: i, out_of_range: o },
                _ => Sampler::RangeChoice {
                    input,
                    min: min.0,
                    max: max.0,
                    in_range: boxed(in_range, ctx),
                    out_of_range: boxed(out_of_range, ctx),
                },
            }
        }
        F::IntervalSelect { input, thresholds, functions } => {
            let input = boxed(input, ctx);
            if thresholds.len() == 1 {
                let below = boxed(functions.first().unwrap(), ctx);
                let above = boxed(functions.last().unwrap(), ctx);
                Sampler::IntervalSelectSingle { input, threshold: thresholds[0].0, below, above }
            } else {
                Sampler::IntervalSelect {
                    input,
                    thresholds: thresholds.iter().map(|t| t.0).collect(),
                    samplers: functions.iter().map(|f| compile(f, ctx)).collect(),
                }
            }
        }
        F::Cache(_) => panic!("Cannot compile cache before it has been deduplicated"),
        F::BlendDensity(input) => Sampler::BlendDensity(boxed(input, ctx)),
        F::Interpolated { input, cell_xz, cell_y } => Sampler::Interpolated {
            input: boxed(input, ctx),
            cell_xz: *cell_xz,
            cell_y: *cell_y,
            cell_xz_inv: 1.0 / *cell_xz as f32,
            cell_y_inv: 1.0 / *cell_y as f32,
        },
        F::Slice(axis, coordinate, input) => {
            if let F::Slice(inner_axis, inner_coordinate, inner_input) = &**input
                && matches!((axis, inner_axis), (Axis::X, Axis::Z) | (Axis::Z, Axis::X))
            {
                let (x, z) = if *axis == Axis::X { (*coordinate, *inner_coordinate) } else { (*inner_coordinate, *coordinate) };
                return Sampler::SliceXz(boxed(inner_input, ctx), x, z);
            }
            let input = boxed(input, ctx);
            match axis {
                Axis::X => Sampler::SliceX(input, *coordinate),
                Axis::Y => Sampler::SliceY(input, *coordinate),
                Axis::Z => Sampler::SliceZ(input, *coordinate),
            }
        }
        F::FindTopSurface { density, upper_bound, lower_bound, cell_height } => {
            let sampler = Sampler::FindTopSurface {
                density: boxed(density, ctx),
                upper_bound: boxed(upper_bound, ctx),
                lower_bound: *lower_bound,
                cell_height: *cell_height,
            };
            Sampler::SliceY(Box::new(sampler), 0)
        }
        F::OldBlendedNoise { xz_scale, y_scale, xz_factor, y_factor, smear_scale_multiplier } => {
            let mut random = ctx.random("minecraft:terrain");
            let xz_multiplier = 684.412 * xz_scale.0;
            let y_multiplier = 684.412 * y_scale.0;
            let limit_smear = y_multiplier * smear_scale_multiplier.0;
            let main_smear = limit_smear / y_factor.0;
            let min_limit = Arc::new(synth::blended_fbm(&mut random, -15, limit_smear, 0.99998474f32 as f64));
            let max_limit = Arc::new(synth::blended_fbm(&mut random, -15, limit_smear, 0.99998474f32 as f64));
            let main = Arc::new(synth::blended_fbm(&mut random, -7, main_smear, 12.75));
            let main = Sampler::Noise { noise: main, xz_scale: xz_multiplier / xz_factor.0, y_scale: y_multiplier / y_factor.0 };
            let choice = Sampler::Clamp(Box::new(Sampler::ConstAdd(Box::new(main), 0.5)), 0.0, 1.0);
            Sampler::Lerp(
                Box::new(choice),
                Box::new(Sampler::Noise { noise: min_limit, xz_scale: xz_multiplier, y_scale: y_multiplier }),
                Box::new(Sampler::Noise { noise: max_limit, xz_scale: xz_multiplier, y_scale: y_multiplier }),
            )
        }
        F::Prepared(prepared) => Sampler::Cache { id: prepared.id, input: prepared.input.clone() },
    }
}
