use serde_json::Value;

use super::ease;
use crate::world::level::levelgen::density_function::float;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EasingType {
    Simple(usize),
    CubicBezier(CubicBezier),
}

const SIMPLE: [(&str, fn(f32) -> f32); 32] = [
    ("constant", |_| 0.0),
    ("linear", |x| x),
    ("in_back", ease::in_back),
    ("in_bounce", ease::in_bounce),
    ("in_circ", ease::in_circ),
    ("in_cubic", ease::in_cubic),
    ("in_elastic", ease::in_elastic),
    ("in_expo", ease::in_expo),
    ("in_quad", ease::in_quad),
    ("in_quart", ease::in_quart),
    ("in_quint", ease::in_quint),
    ("in_sine", ease::in_sine),
    ("in_out_back", ease::in_out_back),
    ("in_out_bounce", ease::in_out_bounce),
    ("in_out_circ", ease::in_out_circ),
    ("in_out_cubic", ease::in_out_cubic),
    ("in_out_elastic", ease::in_out_elastic),
    ("in_out_expo", ease::in_out_expo),
    ("in_out_quad", ease::in_out_quad),
    ("in_out_quart", ease::in_out_quart),
    ("in_out_quint", ease::in_out_quint),
    ("in_out_sine", ease::in_out_sine),
    ("out_back", ease::out_back),
    ("out_bounce", ease::out_bounce),
    ("out_circ", ease::out_circ),
    ("out_cubic", ease::out_cubic),
    ("out_elastic", ease::out_elastic),
    ("out_expo", ease::out_expo),
    ("out_quad", ease::out_quad),
    ("out_quart", ease::out_quart),
    ("out_quint", ease::out_quint),
    ("out_sine", ease::out_sine),
];

impl EasingType {
    pub const CONSTANT: EasingType = EasingType::Simple(0);
    pub const LINEAR: EasingType = EasingType::Simple(1);

    pub fn by_name(name: &str) -> Option<EasingType> {
        SIMPLE.iter().position(|(n, _)| *n == name).map(EasingType::Simple)
    }

    pub fn parse(json: &Value) -> Result<EasingType, String> {
        if let Some(name) = json.as_str() {
            return EasingType::by_name(name).ok_or_else(|| format!("unknown easing {name}"));
        }
        let controls = json.get("cubic_bezier").and_then(Value::as_array).ok_or_else(|| format!("not an easing: {json}"))?;
        let [x1, y1, x2, y2] = controls.iter().map(float).collect::<Result<Vec<_>, _>>()?.try_into().map_err(|_| "cubic_bezier takes 4 floats")?;
        if !(0.0..=1.0).contains(&x1) {
            return Err("x1 must be in range [0; 1]".into());
        }
        if !(0.0..=1.0).contains(&x2) {
            return Err("x2 must be in range [0; 1]".into());
        }
        Ok(EasingType::CubicBezier(CubicBezier::new(x1, y1, x2, y2)))
    }

    pub fn apply(&self, x: f32) -> f32 {
        match self {
            EasingType::Simple(i) => SIMPLE[*i].1(x),
            EasingType::CubicBezier(bezier) => bezier.apply(x),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CubicBezier {
    x_curve: CubicCurve,
    y_curve: CubicCurve,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct CubicCurve {
    a: f32,
    b: f32,
    c: f32,
}

impl CubicCurve {
    fn from_controls(v1: f32, v2: f32) -> CubicCurve {
        CubicCurve { a: 3.0 * v1 - 3.0 * v2 + 1.0, b: -6.0 * v1 + 3.0 * v2, c: 3.0 * v1 }
    }

    fn sample(self, t: f32) -> f32 {
        ((self.a * t + self.b) * t + self.c) * t
    }

    fn sample_gradient(self, t: f32) -> f32 {
        (3.0 * self.a * t + 2.0 * self.b) * t + self.c
    }
}

impl CubicBezier {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> CubicBezier {
        CubicBezier { x_curve: CubicCurve::from_controls(x1, x2), y_curve: CubicCurve::from_controls(y1, y2) }
    }

    pub fn apply(&self, x: f32) -> f32 {
        self.y_curve.sample(self.solve_t(x))
    }

    fn solve_t(&self, x: f32) -> f32 {
        let mut t = x;
        for _ in 0..4 {
            let error = self.x_curve.sample(t) - x;
            if error.abs() < 1.0E-5 {
                return t;
            }
            let gradient = self.x_curve.sample_gradient(t);
            if gradient < 1.0E-5 {
                break;
            }
            t -= (error / gradient).clamp(-0.25, 0.25);
        }
        self.solve_t_bisect(x, t)
    }

    fn solve_t_bisect(&self, x: f32, initial_t: f32) -> f32 {
        let mut t0 = 0.0f32;
        let mut t1 = 1.0f32;
        let mut t = initial_t;
        while t0 < t1 {
            let error = self.x_curve.sample(t) - x;
            if error.abs() < 1.0E-5 {
                return t;
            }
            if error < 0.0 {
                t0 = t;
            } else {
                t1 = t;
            }
            t = (t1 + t0) / 2.0;
        }
        t
    }
}
