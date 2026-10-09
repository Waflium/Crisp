use std::collections::HashMap;

use crisp_core::Interval;
use crisp_core::mth::{jmax_f, jmin_f, lerp_f};
use serde_json::Value;

use super::{Df, F32, Parser, SamplerContext, float};
use super::context::DensityVolume;
use super::sampler::Sampler;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Spline {
    Constant(F32),
    Multipoint { coordinate: Df, locations: Vec<F32>, values: Vec<Spline>, derivatives: Vec<F32> },
}

fn linear_extend(input: f32, locations: &[f32], value: f32, derivatives: &[f32], index: usize) -> f32 {
    let derivative = derivatives[index];
    if derivative == 0.0 { value } else { value + derivative * (input - locations[index]) }
}

impl Spline {
    pub fn parse(parser: &mut Parser, v: &Value) -> Result<Spline, String> {
        if v.is_number() {
            return Ok(Spline::Constant(F32(float(v)?)));
        }
        let coordinate = parser.parse(v.get("coordinate").ok_or("spline without coordinate")?)?;
        let points = v.get("points").and_then(Value::as_array).ok_or("spline without points")?;
        let (mut locations, mut values, mut derivatives) = (Vec::new(), Vec::new(), Vec::new());
        for point in points {
            locations.push(F32(float(point.get("location").ok_or("location")?)?));
            values.push(Spline::parse(parser, point.get("value").ok_or("value")?)?);
            derivatives.push(F32(float(point.get("derivative").ok_or("derivative")?)?));
        }
        if locations.is_empty() {
            return Err("Cannot create a multipoint spline with no points".into());
        }
        Ok(Spline::Multipoint { coordinate, locations, values, derivatives })
    }

    pub fn for_each_coordinate(&self, f: &mut dyn FnMut(&Df)) {
        if let Spline::Multipoint { coordinate, values, .. } = self {
            f(coordinate);
            for value in values {
                value.for_each_coordinate(f);
            }
        }
    }

    pub fn map_coordinates(&self, f: &mut dyn FnMut(&Df) -> Df) -> Spline {
        match self {
            Spline::Constant(v) => Spline::Constant(*v),
            Spline::Multipoint { coordinate, locations, values, derivatives } => {
                let coordinate = f(coordinate);
                Spline::Multipoint {
                    coordinate,
                    locations: locations.clone(),
                    values: values.iter().map(|v| v.map_coordinates(f)).collect(),
                    derivatives: derivatives.clone(),
                }
            }
        }
    }

    pub fn range(&self) -> Interval {
        let Spline::Multipoint { coordinate, locations, values, derivatives } = self else {
            let Spline::Constant(v) = self else { unreachable!() };
            return Interval::of_exact(v.0);
        };
        let locations: Vec<f32> = locations.iter().map(|l| l.0).collect();
        let derivatives: Vec<f32> = derivatives.iter().map(|d| d.0).collect();
        let last = locations.len() - 1;
        let mut min_value = f32::INFINITY;
        let mut max_value = f32::NEG_INFINITY;
        let input = coordinate.range();
        if input.is_nai() {
            return input;
        }
        if input.min() < locations[0] {
            let first = values[0].range();
            let e1 = linear_extend(input.min(), &locations, first.min(), &derivatives, 0);
            let e2 = linear_extend(input.min(), &locations, first.max(), &derivatives, 0);
            min_value = jmin_f(min_value, jmin_f(e1, e2));
            max_value = jmax_f(max_value, jmax_f(e1, e2));
        }
        if input.max() > locations[last] {
            let last_range = values[last].range();
            let e1 = linear_extend(input.max(), &locations, last_range.min(), &derivatives, last);
            let e2 = linear_extend(input.max(), &locations, last_range.max(), &derivatives, last);
            min_value = jmin_f(min_value, jmin_f(e1, e2));
            max_value = jmax_f(max_value, jmax_f(e1, e2));
        }
        let ranges: Vec<Interval> = values.iter().map(Spline::range).collect();
        for range in &ranges {
            min_value = jmin_f(min_value, range.min());
            max_value = jmax_f(max_value, range.max());
        }
        for i in 0..last {
            let x_diff = locations[i + 1] - locations[i];
            let (r1, r2) = (ranges[i], ranges[i + 1]);
            let (min1, max1, min2, max2) = (r1.min(), r1.max(), r2.min(), r2.max());
            let (d1, d2) = (derivatives[i], derivatives[i + 1]);
            if d1 != 0.0 || d2 != 0.0 {
                let p1 = d1 * x_diff;
                let p2 = d2 * x_diff;
                let min_lerp1 = jmin_f(min1, min2);
                let max_lerp1 = jmax_f(max1, max2);
                let min_a = p1 - max2 + min1;
                let max_a = p1 - min2 + max1;
                let min_b = -p2 + min2 - max1;
                let max_b = -p2 + max2 - min1;
                let min_lerp2 = jmin_f(min_a, min_b);
                let max_lerp2 = jmax_f(max_a, max_b);
                min_value = jmin_f(min_value, min_lerp1 + 0.25 * min_lerp2);
                max_value = jmax_f(max_value, max_lerp1 + 0.25 * max_lerp2);
            }
        }
        Interval::of(min_value, max_value)
    }
}

enum Node {
    Constant(f32),
    Multipoint { coordinate: usize, locations: Box<[f32]>, values: Box<[Node]>, derivatives: Box<[f32]> },
}

pub struct SplineSampler {
    root: Node,
    coordinates: Vec<Sampler>,
}

const MAX_COORDINATES: usize = 16;

impl SplineSampler {
    pub fn new(spline: &Spline, compile: &mut dyn FnMut(&Df) -> Sampler) -> SplineSampler {
        let mut indices: HashMap<Df, usize> = HashMap::new();
        let mut coordinates = Vec::new();
        let root = Self::map(spline, &mut indices, &mut coordinates, compile);
        assert!(coordinates.len() <= MAX_COORDINATES, "too many spline coordinates");
        SplineSampler { root, coordinates }
    }

    fn map(spline: &Spline, indices: &mut HashMap<Df, usize>, coordinates: &mut Vec<Sampler>, compile: &mut dyn FnMut(&Df) -> Sampler) -> Node {
        match spline {
            Spline::Constant(v) => Node::Constant(v.0),
            Spline::Multipoint { coordinate, locations, values, derivatives } => {
                let index = match indices.get(coordinate) {
                    Some(&i) => i,
                    None => {
                        let i = indices.len();
                        coordinates.push(compile(coordinate));
                        indices.insert(coordinate.clone(), i);
                        i
                    }
                };
                Node::Multipoint {
                    coordinate: index,
                    locations: locations.iter().map(|l| l.0).collect(),
                    values: values.iter().map(|v| Self::map(v, indices, coordinates, compile)).collect(),
                    derivatives: derivatives.iter().map(|d| d.0).collect(),
                }
            }
        }
    }

    fn sample(node: &Node, input: &mut dyn FnMut(usize) -> f32) -> f32 {
        let Node::Multipoint { coordinate, locations, values, derivatives } = node else {
            let Node::Constant(v) = node else { unreachable!() };
            return *v;
        };
        let x = input(*coordinate);
        let mut from = 0usize;
        let mut len = locations.len();
        while len > 0 {
            let half = len / 2;
            let middle = from + half;
            if x < locations[middle] {
                len = half;
            } else {
                from = middle + 1;
                len -= half + 1;
            }
        }
        let last = locations.len() - 1;
        if from == 0 {
            return linear_extend(x, locations, Self::sample(&values[0], input), derivatives, 0);
        }
        let start = from - 1;
        if start == last {
            return linear_extend(x, locations, Self::sample(&values[last], input), derivatives, last);
        }
        let x1 = locations[start];
        let x2 = locations[start + 1];
        let t = (x - x1) / (x2 - x1);
        let d1 = derivatives[start];
        let d2 = derivatives[start + 1];
        let y1 = Self::sample(&values[start], input);
        let y2 = Self::sample(&values[start + 1], input);
        let a = d1 * (x2 - x1) - (y2 - y1);
        let b = -d2 * (x2 - x1) + (y2 - y1);
        lerp_f(t, y1, y2) + t * (1.0 - t) * lerp_f(t, a, b)
    }

    pub fn sample_volume(&self, ctx: &mut SamplerContext, out: &mut [f32], volume: &DensityVolume) {
        let mut buffers: [Option<Vec<f32>>; MAX_COORDINATES] = Default::default();
        for i in 0..out.len() {
            let value = Self::sample(&self.root, &mut |c| {
                if buffers[c].is_none() {
                    let mut buffer = ctx.acquire(volume.size());
                    self.coordinates[c].sample_volume(ctx, &mut buffer, volume);
                    buffers[c] = Some(buffer);
                }
                buffers[c].as_ref().unwrap()[i]
            });
            out[i] = value;
        }
        for buffer in buffers.into_iter().flatten() {
            ctx.release(buffer);
        }
    }

    pub fn sample_value(&self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32) -> f32 {
        let mut cached = [f32::NAN; MAX_COORDINATES];
        Self::sample(&self.root, &mut |c| {
            if !cached[c].is_nan() {
                return cached[c];
            }
            let value = self.coordinates[c].sample_value(ctx, x, y, z);
            cached[c] = value;
            value
        })
    }
}
