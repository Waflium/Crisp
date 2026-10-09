use md5::{Digest, Md5};

use crate::mth;

pub trait RandomSource {
    fn set_seed(&mut self, seed: i64);
    fn next_int(&mut self) -> i32;
    fn next_int_bounded(&mut self, bound: i32) -> i32;
    fn next_long(&mut self) -> i64;
    fn next_boolean(&mut self) -> bool;
    fn next_float(&mut self) -> f32;
    fn next_double(&mut self) -> f64;
    fn next_gaussian(&mut self) -> f64;

    fn next_int_between_inclusive(&mut self, min: i32, max_inclusive: i32) -> i32 {
        self.next_int_bounded(max_inclusive - min + 1) + min
    }

    fn next_int_range(&mut self, origin: i32, bound: i32) -> i32 {
        assert!(origin < bound, "bound - origin is non positive");
        origin + self.next_int_bounded(bound - origin)
    }

    fn triangle(&mut self, mean: f64, spread: f64) -> f64 {
        mean + spread * (self.next_double() - self.next_double())
    }

    fn triangle_f(&mut self, mean: f32, spread: f32) -> f32 {
        mean + spread * (self.next_float() - self.next_float())
    }

    fn consume_count(&mut self, rounds: i32) {
        for _ in 0..rounds {
            self.next_int();
        }
    }
}

const DOUBLE_MULTIPLIER: f64 = 1.110223e-16f32 as f64;
const FLOAT_MULTIPLIER: f32 = 5.9604645e-8;

#[derive(Clone, Default)]
pub struct Gaussian {
    next_next: f64,
    have_next_next: bool,
}

impl Gaussian {
    pub fn next(&mut self, random: &mut impl FnMut() -> f64) -> f64 {
        if self.have_next_next {
            self.have_next_next = false;
            return self.next_next;
        }
        let (x, y, radius_squared) = loop {
            let x = 2.0 * random() - 1.0;
            let y = 2.0 * random() - 1.0;
            let r = mth::square(x) + mth::square(y);
            if r < 1.0 && r != 0.0 {
                break (x, y, r);
            }
        };
        let multiplier = (-2.0 * radius_squared.ln() / radius_squared).sqrt();
        self.next_next = y * multiplier;
        self.have_next_next = true;
        x * multiplier
    }
}

#[derive(Clone)]
pub struct LegacyRandomSource {
    seed: i64,
    gaussian: Gaussian,
}

const MULTIPLIER: i64 = 25214903917;
const MODULUS_MASK: i64 = (1 << 48) - 1;

impl LegacyRandomSource {
    pub fn new(seed: i64) -> LegacyRandomSource {
        let mut r = LegacyRandomSource { seed: 0, gaussian: Gaussian::default() };
        r.set_seed(seed);
        r
    }

    pub fn create() -> LegacyRandomSource {
        LegacyRandomSource::new(generate_unique_seed())
    }

    pub fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(11) & MODULUS_MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn fork(&mut self) -> LegacyRandomSource {
        LegacyRandomSource::new(self.next_long())
    }

    pub fn fork_positional(&mut self) -> LegacyPositionalRandomFactory {
        LegacyPositionalRandomFactory { seed: self.next_long() }
    }
}

impl RandomSource for LegacyRandomSource {
    fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULTIPLIER) & MODULUS_MASK;
        self.gaussian.have_next_next = false;
    }

    fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "Bound must be positive");
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    fn next_long(&mut self) -> i64 {
        let upper = self.next(32) as i64;
        let lower = self.next(32) as i64;
        (upper << 32).wrapping_add(lower)
    }

    fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * FLOAT_MULTIPLIER
    }

    fn next_double(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        ((upper << 27) + lower) as f64 * DOUBLE_MULTIPLIER
    }

    fn next_gaussian(&mut self) -> f64 {
        let mut gaussian = std::mem::take(&mut self.gaussian);
        let v = gaussian.next(&mut || self.next_double());
        self.gaussian = gaussian;
        v
    }
}

#[derive(Clone, Copy)]
pub struct LegacyPositionalRandomFactory {
    seed: i64,
}

impl LegacyPositionalRandomFactory {
    pub fn at(&self, x: i32, y: i32, z: i32) -> LegacyRandomSource {
        LegacyRandomSource::new(mth::get_seed(x, y, z) ^ self.seed)
    }

    pub fn from_hash_of(&self, name: &str) -> LegacyRandomSource {
        LegacyRandomSource::new(java_string_hash(name) as i64 ^ self.seed)
    }

    pub fn from_seed(&self, seed: i64) -> LegacyRandomSource {
        LegacyRandomSource::new(seed)
    }
}

#[derive(Clone)]
pub struct XoroshiroRandomSource {
    lo: i64,
    hi: i64,
    gaussian: Gaussian,
}

const GOLDEN_RATIO_64: i64 = -7046029254386353131;
const SILVER_RATIO_64: i64 = 7640891576956012809;

pub fn mix_stafford13(mut z: i64) -> i64 {
    z = (z ^ ((z as u64) >> 30) as i64).wrapping_mul(-4658895280553007687);
    z = (z ^ ((z as u64) >> 27) as i64).wrapping_mul(-7723592293110705685);
    z ^ ((z as u64) >> 31) as i64
}

pub fn upgrade_seed_to_128bit(seed: i64) -> (i64, i64) {
    let lo = seed ^ SILVER_RATIO_64;
    let hi = lo.wrapping_add(GOLDEN_RATIO_64);
    (mix_stafford13(lo), mix_stafford13(hi))
}

pub fn seed_from_hash_of(name: &str) -> (i64, i64) {
    let hash = Md5::digest(name.as_bytes());
    (i64::from_be_bytes(hash[..8].try_into().unwrap()), i64::from_be_bytes(hash[8..].try_into().unwrap()))
}

impl XoroshiroRandomSource {
    pub fn new(seed: i64) -> XoroshiroRandomSource {
        let (lo, hi) = upgrade_seed_to_128bit(seed);
        XoroshiroRandomSource::from_parts(lo, hi)
    }

    pub fn from_parts(lo: i64, hi: i64) -> XoroshiroRandomSource {
        let (lo, hi) = if lo | hi == 0 { (GOLDEN_RATIO_64, SILVER_RATIO_64) } else { (lo, hi) };
        XoroshiroRandomSource { lo, hi, gaussian: Gaussian::default() }
    }

    pub fn state(&self) -> (i64, i64) {
        (self.lo, self.hi)
    }

    fn next_raw(&mut self) -> i64 {
        let s0 = self.lo;
        let mut s1 = self.hi;
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.lo = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.hi = s1.rotate_left(28);
        result
    }

    fn next_bits(&mut self, bits: u32) -> i64 {
        ((self.next_raw() as u64) >> (64 - bits)) as i64
    }

    pub fn fork(&mut self) -> XoroshiroRandomSource {
        let lo = self.next_raw();
        let hi = self.next_raw();
        XoroshiroRandomSource::from_parts(lo, hi)
    }

    pub fn fork_positional(&mut self) -> XoroshiroPositionalRandomFactory {
        let lo = self.next_raw();
        let hi = self.next_raw();
        XoroshiroPositionalRandomFactory { lo, hi }
    }
}

impl RandomSource for XoroshiroRandomSource {
    fn set_seed(&mut self, seed: i64) {
        *self = XoroshiroRandomSource::new(seed);
    }

    fn next_int(&mut self) -> i32 {
        self.next_raw() as i32
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "Bound must be positive");
        let bound = bound as i64;
        let mut multiplied = (self.next_int() as u32 as i64) * bound;
        let mut fractional = multiplied & 0xFFFF_FFFF;
        if fractional < bound {
            let unbiased_start = ((!bound + 1) as u32 % bound as u32) as i64;
            while fractional < unbiased_start {
                multiplied = (self.next_int() as u32 as i64) * bound;
                fractional = multiplied & 0xFFFF_FFFF;
            }
        }
        (multiplied >> 32) as i32
    }

    fn next_long(&mut self) -> i64 {
        self.next_raw()
    }

    fn next_boolean(&mut self) -> bool {
        self.next_raw() & 1 != 0
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * FLOAT_MULTIPLIER
    }

    fn next_double(&mut self) -> f64 {
        self.next_bits(53) as f64 * DOUBLE_MULTIPLIER
    }

    fn next_gaussian(&mut self) -> f64 {
        let mut gaussian = std::mem::take(&mut self.gaussian);
        let v = gaussian.next(&mut || self.next_double());
        self.gaussian = gaussian;
        v
    }

    fn consume_count(&mut self, rounds: i32) {
        for _ in 0..rounds {
            self.next_raw();
        }
    }
}

#[derive(Clone, Copy)]
pub struct XoroshiroPositionalRandomFactory {
    lo: i64,
    hi: i64,
}

impl XoroshiroPositionalRandomFactory {
    pub fn at(&self, x: i32, y: i32, z: i32) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_parts(mth::get_seed(x, y, z) ^ self.lo, self.hi)
    }

    pub fn from_hash_of(&self, name: &str) -> XoroshiroRandomSource {
        let (lo, hi) = seed_from_hash_of(name);
        XoroshiroRandomSource::from_parts(lo ^ self.lo, hi ^ self.hi)
    }

    pub fn from_seed(&self, seed: i64) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_parts(seed ^ self.lo, seed ^ self.hi)
    }
}

pub fn java_string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

pub fn generate_unique_seed() -> i64 {
    use std::sync::atomic::{AtomicI64, Ordering};
    static UNIQUIFIER: AtomicI64 = AtomicI64::new(8682522807148012);
    let next = UNIQUIFIER
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |c| Some(c.wrapping_mul(1181783497276652981)))
        .unwrap()
        .wrapping_mul(1181783497276652981);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as i64);
    next ^ nanos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_matches_vanilla() {
        let mut r = LegacyRandomSource::new(42);
        assert_eq!(r.next_int(), -1170105035);
        assert_eq!(r.next_int_bounded(100), 63);
        assert_eq!(r.next_long(), -5843495416241995736);
        assert_eq!(r.next_float(), 0.3087194);
        assert_eq!(r.next_double(), 0.9420735430282128);
        assert_eq!(r.next_int_bounded(7), 3);
        assert_eq!(r.next_gaussian(), 0.2809776380727795);
    }

    #[test]
    fn xoroshiro_matches_vanilla() {
        let mut r = XoroshiroRandomSource::new(42);
        assert_eq!(r.next_int(), 1774573855);
        assert_eq!(r.next_int_bounded(100), 31);
        assert_eq!(r.next_long(), -7542733514721318211);
        assert_eq!(r.next_float(), 0.26502723);
        assert_eq!(r.next_double(), 0.45643019714958855);
        assert_eq!(r.next_int_bounded(7), 2);
        assert_eq!(r.next_gaussian(), -0.6290113589290739);
    }
}
