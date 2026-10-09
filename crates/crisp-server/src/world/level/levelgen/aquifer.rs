use std::collections::HashMap;
use std::sync::Arc;

use crisp_core::RandomSource;
use crisp_core::mth;
use crisp_data::{BlockState, blocks};

use super::density_function::{DensityVolume, Sampler, SamplerContext};
use super::quart_pos;
use super::{PositionalRandomFactory, RandomState};

pub const WAY_BELOW_MIN_Y: i32 = -4064;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FluidStatus {
    pub fluid_level: i32,
    pub fluid_type: BlockState,
}

impl FluidStatus {
    pub fn at(&self, y: i32) -> BlockState {
        if y < self.fluid_level { self.fluid_type } else { blocks::AIR.default_state() }
    }
}

#[derive(Clone, Copy)]
pub struct FluidPicker {
    lava: FluidStatus,
    sea: FluidStatus,
    sea_level: i32,
}

impl FluidPicker {
    pub fn new(sea_level: i32, default_fluid: BlockState) -> FluidPicker {
        FluidPicker {
            lava: FluidStatus { fluid_level: -54, fluid_type: blocks::LAVA.default_state() },
            sea: FluidStatus { fluid_level: sea_level, fluid_type: default_fluid },
            sea_level,
        }
    }

    #[inline(always)]
    pub fn compute_fluid(&self, _x: i32, y: i32, _z: i32) -> FluidStatus {
        if y < (-54).min(self.sea_level) { self.lava } else { self.sea }
    }
}

#[derive(Clone)]
pub struct AquiferSamplers {
    pub barrier: Arc<Sampler>,
    pub fluid_level_floodedness: Arc<Sampler>,
    pub fluid_level_spread: Arc<Sampler>,
    pub lava: Arc<Sampler>,
    pub exclusion: Arc<Sampler>,
    pub surface_level: Arc<Sampler>,
}

pub enum Aquifer {
    Disabled(FluidPicker),
    NoiseBased(Box<NoiseBasedAquifer>),
}

impl Aquifer {
    pub fn compute_substance(&mut self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32, density: f64) -> Option<BlockState> {
        match self {
            Aquifer::Disabled(picker) => {
                if density > 0.0 {
                    None
                } else {
                    Some(picker.compute_fluid(x, y, z).at(y))
                }
            }
            Aquifer::NoiseBased(a) => a.compute_substance(ctx, x, y, z, density),
        }
    }

    pub fn should_schedule_fluid_update(&self) -> bool {
        match self {
            Aquifer::Disabled(_) => false,
            Aquifer::NoiseBased(a) => a.should_schedule_fluid_update,
        }
    }
}

const SURFACE_SAMPLING_OFFSETS_IN_CHUNKS: [(i32, i32); 13] =
    [(0, 0), (-2, -1), (-1, -1), (0, -1), (1, -1), (-3, 0), (-2, 0), (-1, 0), (1, 0), (-2, 1), (-1, 1), (0, 1), (1, 1)];

fn similarity(distance_sqr1: i32, distance_sqr2: i32) -> f64 {
    1.0 - (distance_sqr2 - distance_sqr1) as f64 / 25.0
}

const FLOWING_UPDATE_SIMILARITY: f64 = 1.0 - (144 - 100) as f64 / 25.0;

fn grid_x(c: i32) -> i32 {
    c >> 4
}
fn from_grid_x(c: i32, offset: i32) -> i32 {
    (c << 4) + offset
}
fn grid_y(c: i32) -> i32 {
    c.div_euclid(12)
}
fn from_grid_y(c: i32, offset: i32) -> i32 {
    c * 12 + offset
}

pub struct NoiseBasedAquifer {
    samplers: AquiferSamplers,
    random: PositionalRandomFactory,
    aquifer_cache: Vec<Option<FluidStatus>>,
    location_cache: Vec<i64>,
    picker: FluidPicker,
    should_schedule_fluid_update: bool,
    skip_sampling_above_y: i32,
    min_grid_x: i32,
    min_grid_y: i32,
    min_grid_z: i32,
    grid_size_x: i32,
    grid_size_z: i32,
    surface_level_cache: HashMap<i64, i32>,
}

fn pack(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FF_FFFF) << 38) | (y as i64 & 0xFFF) | ((z as i64 & 0x3FF_FFFF) << 12)
}
fn unpack_x(p: i64) -> i32 {
    (p >> 38) as i32
}
fn unpack_y(p: i64) -> i32 {
    ((p << 52) >> 52) as i32
}
fn unpack_z(p: i64) -> i32 {
    ((p << 26) >> 38) as i32
}

impl NoiseBasedAquifer {
    pub fn new(ctx: &mut SamplerContext, samplers: AquiferSamplers, random: PositionalRandomFactory, volume: &DensityVolume, picker: FluidPicker) -> NoiseBasedAquifer {
        let min_grid_x = grid_x(volume.min_x - 5);
        let max_grid_x = grid_x(volume.max_x() - 5) + 1;
        let grid_size_x = max_grid_x - min_grid_x + 1;
        let min_grid_y = grid_y(volume.min_y + 1) - 1;
        let max_grid_y = grid_y(volume.max_y() + 1) + 1;
        let grid_size_y = max_grid_y - min_grid_y + 1;
        let min_grid_z = grid_x(volume.min_z - 5);
        let max_grid_z = grid_x(volume.max_z() - 5) + 1;
        let grid_size_z = max_grid_z - min_grid_z + 1;
        let total = (grid_size_x * grid_size_y * grid_size_z) as usize;
        let mut aquifer = NoiseBasedAquifer {
            samplers,
            random,
            aquifer_cache: vec![None; total],
            location_cache: vec![i64::MAX; total],
            picker,
            should_schedule_fluid_update: false,
            skip_sampling_above_y: 0,
            min_grid_x,
            min_grid_y,
            min_grid_z,
            grid_size_x,
            grid_size_z,
            surface_level_cache: HashMap::new(),
        };
        let max_surface = aquifer.max_surface_level(
            ctx,
            from_grid_x(min_grid_x, 0),
            from_grid_x(min_grid_z, 0),
            from_grid_x(max_grid_x, 9),
            from_grid_x(max_grid_z, 9),
        );
        let max_adjusted = max_surface + 8;
        let skip_grid_y = grid_y(max_adjusted + 12) + 1;
        aquifer.skip_sampling_above_y = from_grid_y(skip_grid_y, 11) - 1;
        aquifer
    }

    fn surface_level(&mut self, ctx: &mut SamplerContext, x: i32, z: i32) -> i32 {
        let qx = quart_pos::to_block(quart_pos::from_block(x));
        let qz = quart_pos::to_block(quart_pos::from_block(z));
        let key = crisp_core::ChunkPos::new(qx, qz).pack();
        if let Some(&level) = self.surface_level_cache.get(&key) {
            return level;
        }
        let level = mth::floor_f(self.samplers.surface_level.sample_value(ctx, qx, 0, qz));
        self.surface_level_cache.insert(key, level);
        level
    }

    fn max_surface_level(&mut self, ctx: &mut SamplerContext, min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> i32 {
        let (min_qx, max_qx) = (quart_pos::from_block(min_x), quart_pos::from_block(max_x));
        let (min_qz, max_qz) = (quart_pos::from_block(min_z), quart_pos::from_block(max_z));
        let volume =
            DensityVolume::stepped(max_qx - min_qx + 1, 1, max_qz - min_qz + 1, quart_pos::to_block(min_qx), 0, quart_pos::to_block(min_qz), 4, 1, 4);
        let mut buffer = ctx.acquire(volume.size());
        self.samplers.surface_level.sample_volume(ctx, &mut buffer, &volume);
        let mut max_y = i32::MIN;
        for z in 0..volume.size_z {
            for x in 0..volume.size_x {
                let level = mth::floor_f(buffer[volume.index(x, 0, z)]);
                self.surface_level_cache.insert(crisp_core::ChunkPos::new(volume.block_x(x), volume.block_z(z)).pack(), level);
                max_y = max_y.max(level);
            }
        }
        ctx.release(buffer);
        max_y
    }

    fn index(&self, gx: i32, gy: i32, gz: i32) -> usize {
        let (x, y, z) = (gx - self.min_grid_x, gy - self.min_grid_y, gz - self.min_grid_z);
        ((y * self.grid_size_z + z) * self.grid_size_x + x) as usize
    }

    pub fn compute_substance(&mut self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32, density: f64) -> Option<BlockState> {
        if density > 0.0 {
            self.should_schedule_fluid_update = false;
            return None;
        }
        let global = self.picker.compute_fluid(x, y, z);
        if y > self.skip_sampling_above_y {
            self.should_schedule_fluid_update = false;
            return Some(global.at(y));
        }
        if global.at(y).is_block(blocks::LAVA) {
            self.should_schedule_fluid_update = false;
            return Some(blocks::LAVA.default_state());
        }
        let x_anchor = grid_x(x - 5);
        let y_anchor = grid_y(y + 1);
        let z_anchor = grid_x(z - 5);
        let mut d = [i32::MAX; 4];
        let mut closest = [0usize; 4];
        for x1 in 0..=1 {
            for y1 in -1..=1 {
                for z1 in 0..=1 {
                    let (gx, gy, gz) = (x_anchor + x1, y_anchor + y1, z_anchor + z1);
                    let index = self.index(gx, gy, gz);
                    let existing = self.location_cache[index];
                    let location = if existing != i64::MAX {
                        existing
                    } else {
                        let mut random = self.random.at(gx, gy, gz);
                        let lx = from_grid_x(gx, random.next_int_bounded(10));
                        let ly = from_grid_y(gy, random.next_int_bounded(9));
                        let lz = from_grid_x(gz, random.next_int_bounded(10));
                        let location = pack(lx, ly, lz);
                        self.location_cache[index] = location;
                        location
                    };
                    let dx = unpack_x(location) - x;
                    let dy = unpack_y(location) - y;
                    let dz = unpack_z(location) - z;
                    let distance = dx * dx + dy * dy + dz * dz;
                    if d[0] >= distance {
                        closest = [index, closest[0], closest[1], closest[2]];
                        d = [distance, d[0], d[1], d[2]];
                    } else if d[1] >= distance {
                        closest = [closest[0], index, closest[1], closest[2]];
                        d = [d[0], distance, d[1], d[2]];
                    } else if d[2] >= distance {
                        closest[3] = closest[2];
                        closest[2] = index;
                        d[3] = d[2];
                        d[2] = distance;
                    } else if d[3] >= distance {
                        closest[3] = index;
                        d[3] = distance;
                    }
                }
            }
        }
        let status1 = self.status(ctx, closest[0]);
        let similarity12 = similarity(d[0], d[1]);
        let fluid_state = status1.at(y);
        if similarity12 <= 0.0 {
            if similarity12 >= FLOWING_UPDATE_SIMILARITY {
                let status2 = self.status(ctx, closest[1]);
                self.should_schedule_fluid_update = status1 != status2;
            } else {
                self.should_schedule_fluid_update = false;
            }
            return Some(fluid_state);
        }
        if fluid_state.is_block(blocks::WATER) && self.picker.compute_fluid(x, y - 1, z).at(y - 1).is_block(blocks::LAVA) {
            self.should_schedule_fluid_update = true;
            return Some(fluid_state);
        }
        let mut barrier = f64::NAN;
        let status2 = self.status(ctx, closest[1]);
        let barrier12 = similarity12 * self.pressure(ctx, x, y, z, &mut barrier, status1, status2);
        if density + barrier12 > 0.0 {
            self.should_schedule_fluid_update = false;
            return None;
        }
        let status3 = self.status(ctx, closest[2]);
        let similarity13 = similarity(d[0], d[2]);
        if similarity13 > 0.0 {
            let barrier13 = similarity12 * similarity13 * self.pressure(ctx, x, y, z, &mut barrier, status1, status3);
            if density + barrier13 > 0.0 {
                self.should_schedule_fluid_update = false;
                return None;
            }
        }
        let similarity23 = similarity(d[1], d[2]);
        if similarity23 > 0.0 {
            let barrier23 = similarity12 * similarity23 * self.pressure(ctx, x, y, z, &mut barrier, status2, status3);
            if density + barrier23 > 0.0 {
                self.should_schedule_fluid_update = false;
                return None;
            }
        }
        let may_flow12 = status1 != status2;
        let may_flow23 = similarity23 >= FLOWING_UPDATE_SIMILARITY && status2 != status3;
        let may_flow13 = similarity13 >= FLOWING_UPDATE_SIMILARITY && status1 != status3;
        if !may_flow12 && !may_flow23 && !may_flow13 {
            self.should_schedule_fluid_update = similarity13 >= FLOWING_UPDATE_SIMILARITY
                && similarity(d[0], d[3]) >= FLOWING_UPDATE_SIMILARITY
                && status1 != self.status(ctx, closest[3]);
        } else {
            self.should_schedule_fluid_update = true;
        }
        Some(fluid_state)
    }

    #[allow(clippy::too_many_arguments)]
    fn pressure(&self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32, barrier: &mut f64, s1: FluidStatus, s2: FluidStatus) -> f64 {
        let (t1, t2) = (s1.at(y), s2.at(y));
        let lava_water = t1.is_block(blocks::LAVA) && t2.is_block(blocks::WATER);
        let water_lava = t1.is_block(blocks::WATER) && t2.is_block(blocks::LAVA);
        if lava_water || water_lava {
            return 2.0;
        }
        let fluid_y_diff = (s1.fluid_level - s2.fluid_level).abs();
        if fluid_y_diff == 0 {
            return 0.0;
        }
        let average_fluid_y = 0.5 * (s1.fluid_level + s2.fluid_level) as f64;
        let above_average = y as f64 + 0.5 - average_fluid_y;
        let base_value = fluid_y_diff as f64 / 2.0;
        let towards_middle = base_value - above_average.abs();
        let gradient = if above_average > 0.0 {
            let center = 0.0 + towards_middle;
            if center > 0.0 { center / 1.5 } else { center / 2.5 }
        } else {
            let center = 3.0 + towards_middle;
            if center > 0.0 { center / 3.0 } else { center / 10.0 }
        };
        let noise = if !(gradient < -2.0) && !(gradient > 2.0) {
            if barrier.is_nan() {
                *barrier = self.samplers.barrier.sample_value(ctx, x, y, z) as f64;
            }
            *barrier
        } else {
            0.0
        };
        2.0 * (noise + gradient)
    }

    fn status(&mut self, ctx: &mut SamplerContext, index: usize) -> FluidStatus {
        if let Some(status) = self.aquifer_cache[index] {
            return status;
        }
        let location = self.location_cache[index];
        let status = self.compute_fluid(ctx, unpack_x(location), unpack_y(location), unpack_z(location));
        self.aquifer_cache[index] = Some(status);
        status
    }

    fn compute_fluid(&mut self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32) -> FluidStatus {
        let global = self.picker.compute_fluid(x, y, z);
        let mut lowest_surface = i32::MAX;
        let top_of_cell = y + 12;
        let bottom_of_cell = y - 12;
        let mut center_under_fluid = false;
        for (ox, oz) in SURFACE_SAMPLING_OFFSETS_IN_CHUNKS {
            let sample_x = x + (ox << 4);
            let sample_z = z + (oz << 4);
            let surface = self.surface_level(ctx, sample_x, sample_z);
            let adjusted = surface + 8;
            let start = ox == 0 && oz == 0;
            if start && bottom_of_cell > adjusted {
                return global;
            }
            let pokes_above = top_of_cell > adjusted;
            if pokes_above || start {
                let at_surface = self.picker.compute_fluid(sample_x, adjusted, sample_z);
                if !at_surface.at(adjusted).is_air() {
                    if start {
                        center_under_fluid = true;
                    }
                    if pokes_above {
                        return at_surface;
                    }
                }
            }
            lowest_surface = lowest_surface.min(surface);
        }
        let level = self.compute_surface_level(ctx, x, y, z, global, lowest_surface, center_under_fluid);
        FluidStatus { fluid_level: level, fluid_type: self.compute_fluid_type(ctx, x, y, z, global, level) }
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_surface_level(&self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32, global: FluidStatus, lowest: i32, center_under_fluid: bool) -> i32 {
        let (partially, fully);
        if self.samplers.exclusion.sample_value(ctx, x, y, z) > 0.0 {
            partially = -1.0;
            fully = -1.0;
        } else {
            let below_surface = lowest + 8 - y;
            let factor = if center_under_fluid { mth::clamped_map(below_surface as f64, 0.0, 64.0, 1.0, 0.0) } else { 0.0 };
            let floodedness = mth::jclamp(self.samplers.fluid_level_floodedness.sample_value(ctx, x, y, z) as f64, -1.0, 1.0);
            let fully_threshold = mth::map(factor, 1.0, 0.0, -0.3, 0.8);
            let partially_threshold = mth::map(factor, 1.0, 0.0, -0.8, 0.4);
            partially = floodedness - partially_threshold;
            fully = floodedness - fully_threshold;
        }
        if fully > 0.0 {
            global.fluid_level
        } else if partially > 0.0 {
            self.randomized_surface_level(ctx, x, y, z, lowest)
        } else {
            WAY_BELOW_MIN_Y
        }
    }

    fn randomized_surface_level(&self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32, lowest: i32) -> i32 {
        let cell_x = x.div_euclid(16);
        let cell_y = y.div_euclid(40);
        let cell_z = z.div_euclid(16);
        let middle_y = cell_y * 40 + 20;
        let spread = (self.samplers.fluid_level_spread.sample_value(ctx, cell_x, cell_y, cell_z) * 10.0) as f64;
        let quantized = mth::quantize(spread, 3);
        lowest.min(middle_y + quantized)
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_fluid_type(&self, ctx: &mut SamplerContext, x: i32, y: i32, z: i32, global: FluidStatus, level: i32) -> BlockState {
        let mut fluid_type = global.fluid_type;
        if level <= -10 && level != WAY_BELOW_MIN_Y && global.fluid_type != blocks::LAVA.default_state() {
            let lava = self.samplers.lava.sample_value(ctx, x.div_euclid(64), y.div_euclid(40), z.div_euclid(64)) as f64;
            if lava.abs() > 0.3 {
                fluid_type = blocks::LAVA.default_state();
            }
        }
        fluid_type
    }
}

impl AquiferSamplers {
    pub fn new(state: &RandomState, config: &super::AquiferConfig) -> AquiferSamplers {
        AquiferSamplers {
            barrier: state.sampler(&config.barrier),
            fluid_level_floodedness: state.sampler(&config.fluid_level_floodedness),
            fluid_level_spread: state.sampler(&config.fluid_level_spread),
            lava: state.sampler(&config.lava),
            exclusion: state.sampler(&config.exclusion),
            surface_level: state.sampler(&config.surface_level),
        }
    }
}
