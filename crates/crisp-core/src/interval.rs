use crate::mth::{jmax_f, jmin_f, java_signum_f};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    min: f32,
    max: f32,
}

impl Interval {
    pub const NAI: Interval = Interval { min: f32::NAN, max: f32::NAN };
    pub const INFINITE: Interval = Interval { min: f32::NEG_INFINITY, max: f32::INFINITY };

    pub fn of(min: f32, max: f32) -> Interval {
        debug_assert!(!(max < min) && !min.is_nan() && !max.is_nan(), "bad interval [{min}; {max}]");
        Interval { min, max }
    }

    pub fn of_symmetric(range: f32) -> Interval {
        Interval::of(-range, range)
    }

    pub fn of_exact(value: f32) -> Interval {
        Interval::of(value, value)
    }

    pub fn min(self) -> f32 {
        self.min
    }

    pub fn max(self) -> f32 {
        self.max
    }

    pub fn is_nai(self) -> bool {
        self.min.is_nan()
    }

    pub fn contains(self, value: f32) -> bool {
        value >= self.min && value <= self.max
    }

    pub fn encapsulating_all(intervals: impl IntoIterator<Item = Interval>) -> Interval {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for interval in intervals {
            if !interval.is_nai() {
                min = jmin_f(interval.min, min);
                max = jmax_f(interval.max, max);
            }
        }
        if max < min { Interval::NAI } else { Interval::of(min, max) }
    }

    pub fn encapsulating(first: f32, second: f32) -> Interval {
        if first.is_nan() && second.is_nan() {
            Interval::NAI
        } else if first.is_nan() {
            Interval::of_exact(second)
        } else if second.is_nan() {
            Interval::of_exact(first)
        } else {
            Interval::of(jmin_f(first, second), jmax_f(first, second))
        }
    }

    fn encapsulating_with(first: Interval, second: f32) -> Interval {
        if second.is_nan() {
            first
        } else if first.is_nai() {
            Interval::of_exact(second)
        } else {
            Interval::of(jmin_f(first.min, second), jmax_f(first.max, second))
        }
    }

    fn checked(min: f32, max: f32) -> Interval {
        if !min.is_nan() && !max.is_nan() { Interval::of(min, max) } else { Interval::NAI }
    }

    pub fn add(left: Interval, right: Interval) -> Interval {
        Interval::checked(left.min + right.min, left.max + right.max)
    }

    pub fn sub(left: Interval, right: Interval) -> Interval {
        Interval::checked(left.min - right.max, left.max - right.min)
    }

    fn mul_bound(left: f32, right: f32) -> f32 {
        if left != 0.0 && right != 0.0 { left * right } else { 0.0 }
    }

    pub fn mul(left: Interval, right: Interval) -> Interval {
        if left.is_nai() || right.is_nai() {
            return Interval::NAI;
        }
        let min_min = Interval::mul_bound(left.min, right.min);
        let min_max = Interval::mul_bound(left.min, right.max);
        let max_min = Interval::mul_bound(left.max, right.min);
        let max_max = Interval::mul_bound(left.max, right.max);
        Interval::of(
            jmin_f(jmin_f(min_min, min_max), jmin_f(max_min, max_max)),
            jmax_f(jmax_f(min_min, min_max), jmax_f(max_min, max_max)),
        )
    }

    pub fn reciprocal(input: Interval) -> Interval {
        if input.is_nai() || (input.min == 0.0 && input.max == 0.0) {
            Interval::NAI
        } else if !input.contains(0.0) {
            Interval::of(1.0 / input.max, 1.0 / input.min)
        } else if input.max == 0.0 {
            Interval::of(f32::NEG_INFINITY, 1.0 / input.min)
        } else if input.min == 0.0 {
            Interval::of(1.0 / input.max, f32::INFINITY)
        } else {
            Interval::INFINITE
        }
    }

    pub fn div(left: Interval, right: Interval) -> Interval {
        Interval::mul(left, Interval::reciprocal(right))
    }

    pub fn min_of(left: Interval, right: Interval) -> Interval {
        if left.is_nai() || right.is_nai() {
            Interval::NAI
        } else {
            Interval::of(jmin_f(left.min, right.min), jmin_f(left.max, right.max))
        }
    }

    pub fn max_of(left: Interval, right: Interval) -> Interval {
        if left.is_nai() || right.is_nai() {
            Interval::NAI
        } else {
            Interval::of(jmax_f(left.min, right.min), jmax_f(left.max, right.max))
        }
    }

    pub fn clamp(input: Interval, min: f32, max: f32) -> Interval {
        if input.is_nai() {
            Interval::NAI
        } else if input.min >= max {
            Interval::of(max, max)
        } else if input.max <= min {
            Interval::of(min, min)
        } else {
            Interval::of(jmax_f(input.min, min), jmin_f(input.max, max))
        }
    }

    pub fn abs(input: Interval) -> Interval {
        if input.is_nai() {
            return Interval::NAI;
        }
        let max = jmax_f(input.min.abs(), input.max.abs());
        if input.contains(0.0) { Interval::of(0.0, max) } else { Interval::of(jmin_f(input.min.abs(), input.max.abs()), max) }
    }

    pub fn square(input: Interval) -> Interval {
        if input.is_nai() {
            return Interval::NAI;
        }
        let (a, b) = (input.min * input.min, input.max * input.max);
        let max = jmax_f(a, b);
        if input.contains(0.0) { Interval::of(0.0, max) } else { Interval::of(jmin_f(a, b), max) }
    }

    pub fn pow(base: Interval, exponent: Interval) -> Interval {
        if base.is_nai() || exponent.is_nai() {
            return Interval::NAI;
        }
        if base.min == base.max {
            return Interval::pow_base(base.min, exponent);
        }
        let mut result = Interval::encapsulating_all([Interval::pow_base(base.min, exponent), Interval::pow_base(base.max, exponent)]);
        if base.contains(0.0) {
            if base.max > 0.0 {
                result = Interval::encapsulating_all([result, Interval::pow_base(0.0, exponent)]);
            }
            if base.min < 0.0 {
                result = Interval::encapsulating_all([result, Interval::pow_base(-0.0, exponent)]);
            }
        }
        result
    }

    fn pow_base(base: f32, exponent: Interval) -> Interval {
        if base.is_nan() || exponent.is_nai() {
            Interval::NAI
        } else if exponent.min == exponent.max {
            let value = (base as f64).powf(exponent.min as f64) as f32;
            if value.is_nan() { Interval::NAI } else { Interval::of_exact(value) }
        } else if base == 0.0 {
            Interval::mul(Interval::pow_zero_base(exponent), Interval::of_exact(1.0f32.copysign(base)))
        } else if base == 1.0 {
            Interval::of_exact(1.0)
        } else if base > 0.0 {
            if exponent.min.is_finite() && exponent.max.is_finite() {
                Interval::encapsulating((base as f64).powf(exponent.min as f64) as f32, (base as f64).powf(exponent.max as f64) as f32)
            } else {
                Interval::pow_infinite_exponent(base, exponent)
            }
        } else {
            Interval::pow_negative_base(base, exponent)
        }
    }

    fn pow_zero_base(exponent: Interval) -> Interval {
        if exponent.contains(0.0) {
            if exponent.max == 0.0 {
                Interval::of(1.0, f32::INFINITY)
            } else if exponent.min == 0.0 {
                Interval::of(0.0, 1.0)
            } else {
                Interval::of(0.0, f32::INFINITY)
            }
        } else if exponent.max < 0.0 {
            Interval::of_exact(f32::INFINITY)
        } else {
            Interval::of_exact(0.0)
        }
    }

    fn pow_infinite_exponent(base: f32, exponent: Interval) -> Interval {
        let pow = |e: f32| (base as f64).powf(e as f64) as f32;
        if exponent.min.is_infinite() && exponent.max.is_infinite() {
            Interval::of(0.0, f32::INFINITY)
        } else if exponent.min.is_infinite() {
            if base < 1.0 { Interval::of(pow(exponent.max), f32::INFINITY) } else { Interval::of(0.0, pow(exponent.max)) }
        } else if base < 1.0 {
            Interval::of(0.0, pow(exponent.min))
        } else {
            Interval::of(pow(exponent.min), f32::INFINITY)
        }
    }

    fn pow_negative_base(base: f32, exponent: Interval) -> Interval {
        let pow = |e: f32| (base as f64).powf(e as f64) as f32;
        let min_int = (exponent.min as f64).ceil() as f32;
        let max_int = (exponent.max as f64).floor() as f32;
        if max_int < min_int {
            return Interval::NAI;
        }
        let base_to_min = pow(min_int);
        let base_to_max = pow(max_int);
        let mut result = Interval::encapsulating(base_to_min, base_to_max);
        if min_int.is_infinite() {
            result = Interval::encapsulating_with(result, -base_to_min);
        } else if min_int + 1.0 < max_int {
            result = Interval::encapsulating_with(result, pow(min_int + 1.0));
        }
        if max_int.is_infinite() {
            result = Interval::encapsulating_with(result, -base_to_max);
        } else if max_int - 1.0 > min_int {
            result = Interval::encapsulating_with(result, pow(max_int - 1.0));
        }
        result
    }

    pub fn log(input: Interval) -> Interval {
        if input.max < 0.0 {
            return Interval::NAI;
        }
        let clipped = Interval::max_of(input, Interval::of_exact(0.0));
        Interval::map_monotonic(clipped, |x| (x as f64).ln() as f32)
    }

    pub fn map_monotonic(input: Interval, op: impl Fn(f32) -> f32) -> Interval {
        if input.is_nai() {
            return Interval::NAI;
        }
        let (a, b) = (op(input.min), op(input.max));
        assert!(!a.is_nan() && !b.is_nan(), "Monotonic operator should not produce NaN");
        Interval::of(jmin_f(a, b), jmax_f(a, b))
    }

    pub fn lerp(alpha: Interval, first: Interval, second: Interval) -> Interval {
        if alpha.is_nai() || first.is_nai() || second.is_nai() {
            return Interval::NAI;
        }
        Interval::encapsulating_all([
            Interval::lerp_bounds(alpha, first.min, second.min),
            Interval::lerp_bounds(alpha, first.max, second.min),
            Interval::lerp_bounds(alpha, first.min, second.max),
            Interval::lerp_bounds(alpha, first.max, second.max),
        ])
    }

    fn lerp_bounds(alpha: Interval, first: f32, second: f32) -> Interval {
        if alpha.is_nai() || first.is_nan() || second.is_nan() {
            Interval::NAI
        } else if first.is_finite() && second.is_finite() {
            let bound = |a: f32| first + Interval::mul_bound(a, second - first);
            Interval::encapsulating(bound(alpha.min), bound(alpha.max))
        } else if first == second {
            Interval::of_exact(first)
        } else {
            let bound = |a: f32| {
                let first_part = Interval::mul_bound(1.0 - a, first);
                let second_part = Interval::mul_bound(a, second);
                if !first_part.is_infinite() || !second_part.is_infinite() {
                    first_part + second_part
                } else if a <= 0.0 {
                    if second > first { f32::NEG_INFINITY } else { f32::INFINITY }
                } else if a >= 1.0 {
                    if second > first { f32::INFINITY } else { f32::NEG_INFINITY }
                } else {
                    f32::NAN
                }
            };
            let (a, b) = (bound(alpha.min), bound(alpha.max));
            if !a.is_nan() && !b.is_nan() { Interval::encapsulating(a, b) } else { Interval::NAI }
        }
    }

    pub fn sign(input: Interval) -> Interval {
        if input.is_nai() {
            Interval::NAI
        } else if input.min == input.max {
            Interval::of_exact(java_signum_f(input.min))
        } else if input.contains(0.0) {
            if input.min == 0.0 {
                Interval::of(0.0, 1.0)
            } else if input.max == 0.0 {
                Interval::of(-1.0, 0.0)
            } else {
                Interval::of(-1.0, 1.0)
            }
        } else {
            Interval::of_exact(if input.min > 0.0 { 1.0 } else { -1.0 })
        }
    }
}
