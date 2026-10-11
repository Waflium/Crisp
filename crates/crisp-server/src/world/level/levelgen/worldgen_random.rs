use crisp_core::RandomSource;
use crisp_core::random::{Gaussian, LegacyRandomSource, XoroshiroRandomSource};

use super::AlgorithmRandom;

const FLOAT_MULTIPLIER: f32 = 5.9604645e-8;
const DOUBLE_MULTIPLIER: f64 = 1.110223e-16f32 as f64;

#[derive(Clone)]
pub struct WorldgenRandom {
    random: AlgorithmRandom,
    gaussian: Gaussian,
}

impl WorldgenRandom {
    pub fn new(random: AlgorithmRandom) -> WorldgenRandom {
        WorldgenRandom { random, gaussian: Gaussian::default() }
    }

    pub fn legacy(seed: i64) -> WorldgenRandom {
        WorldgenRandom::new(AlgorithmRandom::Legacy(LegacyRandomSource::new(seed)))
    }

    pub fn next(&mut self, bits: u32) -> i32 {
        match &mut self.random {
            AlgorithmRandom::Legacy(r) => r.next(bits),
            AlgorithmRandom::Xoroshiro(r) => ((r.next_long() as u64) >> (64 - bits)) as i32,
        }
    }

    pub fn xoroshiro(seed: i64) -> WorldgenRandom {
        WorldgenRandom::new(AlgorithmRandom::Xoroshiro(XoroshiroRandomSource::new(seed)))
    }

    pub fn set_decoration_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
        self.set_seed(seed);
        let x_scale = self.next_long() | 1;
        let z_scale = self.next_long() | 1;
        let result = (chunk_x as i64).wrapping_mul(x_scale).wrapping_add((chunk_z as i64).wrapping_mul(z_scale)) ^ seed;
        self.set_seed(result);
        result
    }

    pub fn set_feature_seed(&mut self, seed: i64, index: i32, step: i32) {
        self.set_seed(seed.wrapping_add(index as i64).wrapping_add((10000 * step) as i64));
    }

    pub fn set_large_feature_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(seed);
        let x_scale = self.next_long();
        let z_scale = self.next_long();
        self.set_seed((chunk_x as i64).wrapping_mul(x_scale) ^ (chunk_z as i64).wrapping_mul(z_scale) ^ seed);
    }
}

impl RandomSource for WorldgenRandom {
    fn set_seed(&mut self, seed: i64) {
        self.random.set_seed(seed);
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
