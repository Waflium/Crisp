mod compiler;
mod context;
mod sampler;
mod spline;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crisp_core::Interval;
use crisp_core::mth::jmax_f;
use serde_json::Value;

pub use compiler::{CompileContext, DensityFunctionCompiler};
pub use context::{DensityVolume, SamplerContext};
pub use sampler::Sampler;
pub use spline::Spline;

use super::synth::{self, NormalNoise};

pub const AXIS_X: u8 = 1;
pub const AXIS_Y: u8 = 2;
pub const AXIS_Z: u8 = 4;
pub const ALL_AXES: u8 = 7;

#[derive(Clone, Copy, Debug)]
pub struct F32(pub f32);

impl PartialEq for F32 {
    fn eq(&self, other: &F32) -> bool {
        canonical_f32(self.0) == canonical_f32(other.0)
    }
}
impl Eq for F32 {}
impl Hash for F32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        canonical_f32(self.0).hash(state)
    }
}

fn canonical_f32(v: f32) -> u32 {
    if v.is_nan() { 0x7fc0_0000 } else { v.to_bits() }
}

#[derive(Clone, Copy, Debug)]
pub struct F64(pub f64);

impl PartialEq for F64 {
    fn eq(&self, other: &F64) -> bool {
        canonical_f64(self.0) == canonical_f64(other.0)
    }
}
impl Eq for F64 {}
impl Hash for F64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        canonical_f64(self.0).hash(state)
    }
}

fn canonical_f64(v: f64) -> u64 {
    if v.is_nan() { 0x7ff8_0000_0000_0000 } else { v.to_bits() }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn bit(self) -> u8 {
        match self {
            Axis::X => AXIS_X,
            Axis::Y => AXIS_Y,
            Axis::Z => AXIS_Z,
        }
    }

    pub fn choose(self, x: i32, y: i32, z: i32) -> i32 {
        match self {
            Axis::X => x,
            Axis::Y => y,
            Axis::Z => z,
        }
    }

    fn by_name(name: &str) -> Option<Axis> {
        match name {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TilingMode {
    ClampToEdge,
    Repeat,
    MirroredRepeat,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DistanceMetric {
    Euclidean,
    EuclideanSquared,
    Manhattan,
    Chebyshev,
}

impl DistanceMetric {
    pub fn compute(self, dx: f32, dy: f32, dz: f32) -> f32 {
        match self {
            DistanceMetric::Euclidean => ((dx * dx + dy * dy + dz * dz) as f64).sqrt() as f32,
            DistanceMetric::EuclideanSquared => dx * dx + dy * dy + dz * dz,
            DistanceMetric::Manhattan => dx.abs() + dy.abs() + dz.abs(),
            DistanceMetric::Chebyshev => jmax_f(jmax_f(dx.abs(), dy.abs()), dz.abs()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UnaryType {
    Abs,
    Square,
    Cube,
    Sqrt,
    HalfNegative,
    QuarterNegative,
    Reciprocal,
    Negate,
    Squeeze,
    Log,
    Sign,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RoundType {
    Floor,
    Round,
    Ceil,
    Truncate,
}

impl RoundType {
    pub fn apply(self, input: f32) -> f32 {
        match self {
            RoundType::Floor => (input as f64).floor() as f32,
            RoundType::Round => crisp_core::mth::java_round_f32(input) as f32,
            RoundType::Ceil => (input as f64).ceil() as f32,
            RoundType::Truncate => {
                if input > 0.0 {
                    (input as f64).floor() as f32
                } else {
                    (input as f64).ceil() as f32
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BinaryType {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SimpleType {
    BlendAlpha,
    BlendOffset,
    Beardifier,
}

#[derive(Clone)]
pub struct NoiseHolder {
    pub name: Arc<str>,
    pub noise: Arc<NormalNoise>,
}

impl PartialEq for NoiseHolder {
    fn eq(&self, other: &NoiseHolder) -> bool {
        self.name == other.name
    }
}
impl Eq for NoiseHolder {}
impl Hash for NoiseHolder {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

#[derive(Clone)]
pub struct Reference {
    pub name: Arc<str>,
    pub value: Df,
}

impl PartialEq for Reference {
    fn eq(&self, other: &Reference) -> bool {
        self.name == other.name
    }
}
impl Eq for Reference {}
impl Hash for Reference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

pub struct PreparedCache {
    pub id: usize,
    pub range: Interval,
    pub domain_axes: u8,
    pub input: Arc<Sampler>,
}

impl PartialEq for PreparedCache {
    fn eq(&self, other: &PreparedCache) -> bool {
        self.id == other.id
    }
}
impl Eq for PreparedCache {}
impl Hash for PreparedCache {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

pub type Df = Arc<DensityFunction>;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum DensityFunction {
    Constant(F32),
    Reference(Reference),
    Noise { noise: NoiseHolder, xz_scale: F64, y_scale: F64, shift_x: Df, shift_y: Df, shift_z: Df },
    ShiftA(NoiseHolder),
    ShiftB(NoiseHolder),
    Shift(NoiseHolder),
    Gradient { axis: Axis, tiling: TilingMode, from: i32, to: i32, from_value: F32, to_value: F32 },
    DistanceToPoint { x: i32, y: i32, z: i32, metric: DistanceMetric },
    EndIslands,
    Simple(SimpleType),
    Unary(UnaryType, Df),
    Round(RoundType, Df, Df),
    Binary(BinaryType, Df, Df),
    Pow(Df, Df),
    Spline(Arc<Spline>),
    Lerp(Df, Df, Df),
    Clamp(Df, F32, F32),
    RangeChoice { input: Df, min: F32, max: F32, in_range: Df, out_of_range: Df },
    IntervalSelect { input: Df, thresholds: Vec<F32>, functions: Vec<Df> },
    Cache(Df),
    BlendDensity(Df),
    Interpolated { input: Df, cell_xz: i32, cell_y: i32 },
    Slice(Axis, i32, Df),
    FindTopSurface { density: Df, upper_bound: Df, lower_bound: i32, cell_height: i32 },
    OldBlendedNoise { xz_scale: F64, y_scale: F64, xz_factor: F64, y_factor: F64, smear_scale_multiplier: F64 },
    Prepared(Arc<PreparedCache>),
}

pub fn constant(value: f32) -> Df {
    Arc::new(DensityFunction::Constant(F32(value)))
}

impl DensityFunction {
    pub fn as_constant(&self) -> Option<f32> {
        match self {
            DensityFunction::Constant(v) => Some(v.0),
            _ => None,
        }
    }

    pub fn is_zero(&self) -> bool {
        *self == DensityFunction::Constant(F32(0.0))
    }

    pub fn range(&self) -> Interval {
        use DensityFunction::*;
        match self {
            Constant(v) => Interval::of_exact(v.0),
            Reference(r) => r.value.range(),
            Noise { noise, .. } => noise.noise.range(),
            ShiftA(n) | ShiftB(n) | Shift(n) => Interval::mul(n.noise.range(), Interval::of_exact(4.0)),
            Gradient { from_value, to_value, .. } => Interval::encapsulating(from_value.0, to_value.0),
            DistanceToPoint { .. } => Interval::of(0.0, f32::INFINITY),
            EndIslands => Interval::of(-0.84375, 0.5625),
            Simple(SimpleType::BlendAlpha) => Interval::of(0.0, 1.0),
            Simple(_) => Interval::INFINITE,
            Unary(kind, input) => {
                let input = input.range();
                match kind {
                    UnaryType::Abs => Interval::abs(input),
                    UnaryType::Square => Interval::square(input),
                    UnaryType::Cube => Interval::map_monotonic(input, |v| v * v * v),
                    UnaryType::Sqrt => Interval::pow(input, Interval::of_exact(0.5)),
                    UnaryType::HalfNegative => Interval::map_monotonic(input, |v| leaky_relu(0.5, v)),
                    UnaryType::QuarterNegative => Interval::map_monotonic(input, |v| leaky_relu(0.25, v)),
                    UnaryType::Reciprocal => Interval::reciprocal(input),
                    UnaryType::Negate => Interval::sub(Interval::of_exact(0.0), input),
                    UnaryType::Squeeze => Interval::map_monotonic(input, squeeze),
                    UnaryType::Log => Interval::log(input),
                    UnaryType::Sign => Interval::sign(input),
                }
            }
            Round(kind, input, multiple) => {
                let multiple = multiple.range();
                Interval::mul(Interval::map_monotonic(Interval::div(input.range(), multiple), |v| kind.apply(v)), multiple)
            }
            Binary(kind, left, right) => {
                let (left, right) = (left.range(), right.range());
                match kind {
                    BinaryType::Add => Interval::add(left, right),
                    BinaryType::Sub => Interval::sub(left, right),
                    BinaryType::Mul => Interval::mul(left, right),
                    BinaryType::Div => Interval::div(left, right),
                    BinaryType::Min => Interval::min_of(left, right),
                    BinaryType::Max => Interval::max_of(left, right),
                }
            }
            Pow(base, exponent) => Interval::pow(base.range(), exponent.range()),
            Spline(spline) => spline.range(),
            Lerp(alpha, first, second) => Interval::lerp(alpha.range(), first.range(), second.range()),
            Clamp(input, min, max) => Interval::clamp(input.range(), min.0, max.0),
            RangeChoice { in_range, out_of_range, .. } => Interval::encapsulating_all([in_range.range(), out_of_range.range()]),
            IntervalSelect { functions, .. } => Interval::encapsulating_all(functions.iter().map(|f| f.range())),
            Cache(input) | BlendDensity(input) | Slice(_, _, input) => input.range(),
            Interpolated { input, .. } => input.range(),
            FindTopSurface { upper_bound, lower_bound, .. } => {
                Interval::of(*lower_bound as f32, jmax_f(*lower_bound as f32, upper_bound.range().max()))
            }
            OldBlendedNoise { y_scale, smear_scale_multiplier, .. } => {
                synth::blended_fbm_range(-15, 684.412 * y_scale.0 * smear_scale_multiplier.0, 0.99998474f32 as f64)
            }
            Prepared(p) => p.range,
        }
    }

    pub fn domain_axes(&self) -> u8 {
        use DensityFunction::*;
        match self {
            Constant(_) => 0,
            Reference(r) => r.value.domain_axes(),
            Noise { xz_scale, y_scale, shift_x, shift_y, shift_z, .. } => {
                let mut axes = ALL_AXES;
                if y_scale.0 == 0.0 {
                    axes &= !AXIS_Y;
                }
                if xz_scale.0 == 0.0 {
                    axes &= !(AXIS_X | AXIS_Z);
                }
                axes | shift_x.domain_axes() | shift_y.domain_axes() | shift_z.domain_axes()
            }
            Shift(_) => ALL_AXES,
            ShiftA(_) | ShiftB(_) => AXIS_X | AXIS_Z,
            Gradient { axis, .. } => axis.bit(),
            DistanceToPoint { .. } => ALL_AXES,
            EndIslands => AXIS_X | AXIS_Z,
            Simple(SimpleType::Beardifier) => ALL_AXES,
            Simple(_) => AXIS_X | AXIS_Z,
            Unary(_, input) | Cache(input) | BlendDensity(input) | Clamp(input, _, _) => input.domain_axes(),
            Interpolated { input, .. } => input.domain_axes(),
            Round(_, a, b) | Binary(_, a, b) | Pow(a, b) => a.domain_axes() | b.domain_axes(),
            Spline(spline) => {
                let mut axes = 0;
                spline.for_each_coordinate(&mut |f| axes |= f.domain_axes());
                axes
            }
            Lerp(a, b, c) => a.domain_axes() | b.domain_axes() | c.domain_axes(),
            RangeChoice { input, in_range, out_of_range, .. } => input.domain_axes() | in_range.domain_axes() | out_of_range.domain_axes(),
            IntervalSelect { input, functions, .. } => functions.iter().fold(input.domain_axes(), |a, f| a | f.domain_axes()),
            Slice(axis, _, input) => input.domain_axes() & !axis.bit(),
            FindTopSurface { density, upper_bound, .. } => (density.domain_axes() | upper_bound.domain_axes()) & !AXIS_Y,
            OldBlendedNoise { .. } => ALL_AXES,
            Prepared(p) => p.domain_axes,
        }
    }

    pub fn rewrite_children(self: &Df, rule: &mut dyn FnMut(&Df) -> Df) -> Df {
        use DensityFunction::*;
        let new = match &**self {
            Constant(_) | ShiftA(_) | ShiftB(_) | Shift(_) | Gradient { .. } | DistanceToPoint { .. } | EndIslands | Simple(_) => {
                return self.clone();
            }
            OldBlendedNoise { .. } | Prepared(_) => return self.clone(),
            Reference(r) => {
                let new = rule(&r.value);
                return if Arc::ptr_eq(&new, &r.value) { self.clone() } else { new };
            }
            Noise { noise, xz_scale, y_scale, shift_x, shift_y, shift_z } => {
                let (sx, sy, sz) = (rule(shift_x), rule(shift_y), rule(shift_z));
                Noise { noise: noise.clone(), xz_scale: *xz_scale, y_scale: *y_scale, shift_x: sx, shift_y: sy, shift_z: sz }
            }
            Unary(kind, input) => Unary(*kind, rule(input)),
            Round(kind, input, multiple) => {
                let input = rule(input);
                Round(*kind, input, rule(multiple))
            }
            Binary(kind, left, right) => {
                let left = rule(left);
                Binary(*kind, left, rule(right))
            }
            Pow(base, exponent) => {
                let base = rule(base);
                Pow(base, rule(exponent))
            }
            Spline(spline) => Spline(Arc::new(spline.map_coordinates(rule))),
            Lerp(alpha, first, second) => {
                let alpha = rule(alpha);
                let first = rule(first);
                Lerp(alpha, first, rule(second))
            }
            Clamp(input, min, max) => Clamp(rule(input), *min, *max),
            RangeChoice { input, min, max, in_range, out_of_range } => {
                let input = rule(input);
                let in_range = rule(in_range);
                RangeChoice { input, min: *min, max: *max, in_range, out_of_range: rule(out_of_range) }
            }
            IntervalSelect { input, thresholds, functions } => {
                let input = rule(input);
                IntervalSelect { input, thresholds: thresholds.clone(), functions: functions.iter().map(|f| rule(f)).collect() }
            }
            Cache(input) => Cache(rule(input)),
            BlendDensity(input) => BlendDensity(rule(input)),
            Interpolated { input, cell_xz, cell_y } => Interpolated { input: rule(input), cell_xz: *cell_xz, cell_y: *cell_y },
            Slice(axis, coordinate, input) => Slice(*axis, *coordinate, rule(input)),
            FindTopSurface { density, upper_bound, lower_bound, cell_height } => {
                let density = rule(density);
                FindTopSurface { density, upper_bound: rule(upper_bound), lower_bound: *lower_bound, cell_height: *cell_height }
            }
        };
        Arc::new(new)
    }
}

pub fn leaky_relu(negative_factor: f32, input: f32) -> f32 {
    if input > 0.0 { input } else { input * negative_factor }
}

pub fn squeeze(input: f32) -> f32 {
    let c = crisp_core::mth::jclamp_f(input, -1.0, 1.0);
    c / 2.0 - c * c * c / 24.0
}

pub struct Parser<'a> {
    pub noises: &'a HashMap<String, Arc<NormalNoise>>,
    pub sources: &'a HashMap<&'static str, &'static str>,
    pub parsed: HashMap<String, Df>,
}

pub fn float(v: &Value) -> Result<f32, String> {
    let d = v.as_f64().ok_or_else(|| format!("expected a number, got {v}"))?;
    format!("{d}").parse::<f32>().map_err(|e| e.to_string())
}

fn double(v: &Value) -> Result<f64, String> {
    v.as_f64().ok_or_else(|| format!("expected a number, got {v}"))
}

fn int(v: &Value) -> Result<i32, String> {
    v.as_i64().map(|i| i as i32).ok_or_else(|| format!("expected an integer, got {v}"))
}

pub fn resource_name(name: &str) -> String {
    if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") }
}

impl Parser<'_> {
    pub fn named(&mut self, name: &str) -> Result<Df, String> {
        let name = resource_name(name);
        if let Some(f) = self.parsed.get(&name) {
            return Ok(f.clone());
        }
        let source = self.sources.get(name.as_str()).ok_or_else(|| format!("unknown density function {name}"))?;
        let json: Value = serde_json::from_str(source).map_err(|e| format!("{name}: {e}"))?;
        let f = self.parse(&json).map_err(|e| format!("{name}: {e}"))?;
        self.parsed.insert(name, f.clone());
        Ok(f)
    }

    fn noise(&self, v: &Value) -> Result<NoiseHolder, String> {
        let name = resource_name(v.as_str().ok_or("inline noises are not supported")?);
        let noise = self.noises.get(&name).ok_or_else(|| format!("unknown noise {name}"))?.clone();
        Ok(NoiseHolder { name: name.into(), noise })
    }

    pub fn parse(&mut self, v: &Value) -> Result<Df, String> {
        use DensityFunction::*;
        if let Some(name) = v.as_str() {
            let value = self.named(name)?;
            return Ok(Arc::new(Reference(self::Reference { name: resource_name(name).into(), value })));
        }
        if v.is_number() {
            return Ok(constant(float(v)?));
        }
        let field = |key: &str| v.get(key).ok_or_else(|| format!("missing {key}"));
        let kind = v.get("type").and_then(Value::as_str).ok_or("missing type")?;
        let kind = kind.strip_prefix("minecraft:").unwrap_or(kind);
        let function = match kind {
            "constant" => Constant(F32(float(field("value")?)?)),
            "blend_alpha" => Simple(SimpleType::BlendAlpha),
            "blend_offset" => Simple(SimpleType::BlendOffset),
            "beardifier" => Simple(SimpleType::Beardifier),
            "noise" => {
                let optional = |p: &mut Self, key: &str| match v.get(key) {
                    Some(s) => p.parse(s),
                    None => Ok(constant(0.0)),
                };
                Noise {
                    noise: self.noise(field("noise")?)?,
                    xz_scale: F64(double(field("xz_scale")?)?),
                    y_scale: F64(double(field("y_scale")?)?),
                    shift_x: optional(self, "shift_x")?,
                    shift_y: optional(self, "shift_y")?,
                    shift_z: optional(self, "shift_z")?,
                }
            }
            "end_outer_islands" => EndIslands,
            "distance_to_point" => {
                let p = field("point")?.as_array().ok_or("point must be a list")?;
                let metric = match field("metric")?.as_str() {
                    Some("euclidean") => DistanceMetric::Euclidean,
                    Some("euclidean_squared") => DistanceMetric::EuclideanSquared,
                    Some("manhattan") => DistanceMetric::Manhattan,
                    Some("chebyshev") => DistanceMetric::Chebyshev,
                    m => return Err(format!("unknown metric {m:?}")),
                };
                DistanceToPoint { x: int(&p[0])?, y: int(&p[1])?, z: int(&p[2])?, metric }
            }
            "gradient" => {
                let tiling = match v.get("tiling").and_then(Value::as_str) {
                    None | Some("clamp_to_edge") => TilingMode::ClampToEdge,
                    Some("repeat") => TilingMode::Repeat,
                    Some("mirrored_repeat") => TilingMode::MirroredRepeat,
                    Some(t) => return Err(format!("unknown tiling {t}")),
                };
                Gradient {
                    axis: field("axis")?.as_str().and_then(Axis::by_name).ok_or("bad axis")?,
                    tiling,
                    from: int(field("from_coordinate")?)?,
                    to: int(field("to_coordinate")?)?,
                    from_value: F32(float(field("from_value")?)?),
                    to_value: F32(float(field("to_value")?)?),
                }
            }
            "shift_a" => ShiftA(self.noise(field("noise")?)?),
            "shift_b" => ShiftB(self.noise(field("noise")?)?),
            "shift" => Shift(self.noise(field("noise")?)?),
            "abs" | "square" | "cube" | "sqrt" | "half_negative" | "quarter_negative" | "reciprocal" | "negate" | "squeeze" | "log"
            | "sign" => {
                let t = match kind {
                    "abs" => UnaryType::Abs,
                    "square" => UnaryType::Square,
                    "cube" => UnaryType::Cube,
                    "sqrt" => UnaryType::Sqrt,
                    "half_negative" => UnaryType::HalfNegative,
                    "quarter_negative" => UnaryType::QuarterNegative,
                    "reciprocal" => UnaryType::Reciprocal,
                    "negate" => UnaryType::Negate,
                    "squeeze" => UnaryType::Squeeze,
                    "log" => UnaryType::Log,
                    _ => UnaryType::Sign,
                };
                Unary(t, self.parse(field("input")?)?)
            }
            "floor" | "round" | "ceil" | "truncate" => {
                let t = match kind {
                    "floor" => RoundType::Floor,
                    "round" => RoundType::Round,
                    "ceil" => RoundType::Ceil,
                    _ => RoundType::Truncate,
                };
                let input = self.parse(field("input")?)?;
                let multiple = match v.get("multiple") {
                    Some(m) => self.parse(m)?,
                    None => constant(1.0),
                };
                Round(t, input, multiple)
            }
            "add" | "sub" | "mul" | "div" | "min" | "max" => {
                let t = match kind {
                    "add" => BinaryType::Add,
                    "sub" => BinaryType::Sub,
                    "mul" => BinaryType::Mul,
                    "div" => BinaryType::Div,
                    "min" => BinaryType::Min,
                    _ => BinaryType::Max,
                };
                let left = self.parse(field("left")?)?;
                Binary(t, left, self.parse(field("right")?)?)
            }
            "pow" => {
                let base = self.parse(field("base")?)?;
                Pow(base, self.parse(field("exponent")?)?)
            }
            "spline" => Spline(Arc::new(spline::Spline::parse(self, field("spline")?)?)),
            "lerp" => {
                let alpha = self.parse(field("alpha")?)?;
                let first = self.parse(field("first")?)?;
                Lerp(alpha, first, self.parse(field("second")?)?)
            }
            "clamp" => Clamp(self.parse(field("input")?)?, F32(float(field("min")?)?), F32(float(field("max")?)?)),
            "range_choice" => RangeChoice {
                input: self.parse(field("input")?)?,
                min: F32(float(field("min_inclusive")?)?),
                max: F32(float(field("max_exclusive")?)?),
                in_range: self.parse(field("when_in_range")?)?,
                out_of_range: self.parse(field("when_out_of_range")?)?,
            },
            "interval_select" => {
                let input = self.parse(field("input")?)?;
                let thresholds =
                    field("thresholds")?.as_array().ok_or("thresholds")?.iter().map(|t| float(t).map(F32)).collect::<Result<_, _>>()?;
                let functions = field("functions")?.as_array().ok_or("functions")?.iter().map(|f| self.parse(f)).collect::<Result<_, _>>()?;
                IntervalSelect { input, thresholds, functions }
            }
            "cache" => Cache(self.parse(field("input")?)?),
            "blend_density" => BlendDensity(self.parse(field("input")?)?),
            "interpolated" => Interpolated {
                input: self.parse(field("input")?)?,
                cell_xz: int(field("cell_size_xz")?)?,
                cell_y: int(field("cell_size_y")?)?,
            },
            "slice" => {
                let axis = field("axis")?.as_str().and_then(Axis::by_name).ok_or("bad axis")?;
                Slice(axis, int(field("coordinate")?)?, self.parse(field("input")?)?)
            }
            "find_top_surface" => FindTopSurface {
                density: self.parse(field("density")?)?,
                upper_bound: self.parse(field("upper_bound")?)?,
                lower_bound: int(field("lower_bound")?)?,
                cell_height: int(field("cell_height")?)?,
            },
            "old_blended_noise" => OldBlendedNoise {
                xz_scale: F64(double(field("xz_scale")?)?),
                y_scale: F64(double(field("y_scale")?)?),
                xz_factor: F64(double(field("xz_factor")?)?),
                y_factor: F64(double(field("y_factor")?)?),
                smear_scale_multiplier: F64(double(field("smear_scale_multiplier")?)?),
            },
            other => return Err(format!("unknown density function type {other}")),
        };
        Ok(Arc::new(function))
    }
}
