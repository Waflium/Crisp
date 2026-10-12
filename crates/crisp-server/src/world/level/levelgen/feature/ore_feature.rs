use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use super::abstract_ore_feature::AbstractOreFeature;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::chunk::HeightmapType;

pub struct OreFeature {
    pub ore: AbstractOreFeature,
}

impl OreFeature {
    pub fn parse(json: &Value) -> Result<OreFeature, String> {
        Ok(OreFeature { ore: AbstractOreFeature::parse(json)? })
    }

    pub fn place(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, origin: BlockPos) -> bool {
        let size = self.ore.size;
        let dir = random.next_float() * std::f64::consts::PI as f32;
        let spread_xy = size as f32 / 8.0;
        let max_radius = mth::ceil(((size as f32 / 16.0 * 2.0 + 1.0) / 2.0) as f64);
        let x0 = origin.x as f64 + (dir as f64).sin() * spread_xy as f64;
        let x1 = origin.x as f64 - (dir as f64).sin() * spread_xy as f64;
        let z0 = origin.z as f64 + (dir as f64).cos() * spread_xy as f64;
        let z1 = origin.z as f64 - (dir as f64).cos() * spread_xy as f64;
        let y0 = (origin.y + random.next_int_bounded(3) - 2) as f64;
        let y1 = (origin.y + random.next_int_bounded(3) - 2) as f64;
        let spread = mth::ceil(spread_xy as f64);
        let x_start = origin.x - spread - max_radius;
        let y_start = origin.y - 2 - max_radius;
        let z_start = origin.z - spread - max_radius;
        let size_xz = 2 * (spread + max_radius);
        let size_y = 2 * (2 + max_radius);
        for xprobe in x_start..=x_start + size_xz {
            for zprobe in z_start..=z_start + size_xz {
                if y_start <= level.get_height(HeightmapType::OceanFloorWg, xprobe, zprobe) {
                    return self.do_place(level, random, [x0, x1, z0, z1, y0, y1], x_start, y_start, z_start, size_xz, size_y);
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn do_place(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, [x0, x1, z0, z1, y0, y1]: [f64; 6], x_start: i32, y_start: i32, z_start: i32, size_xz: i32, size_y: i32) -> bool {
        let size = self.ore.size as usize;
        let mut placed = 0;
        let mut tested = vec![false; (size_xz * size_y * size_xz) as usize];
        let mut data = vec![0.0f64; size * 4];
        for i in 0..size {
            let step = i as f32 / size as f32;
            let xx = mth::lerp(step as f64, x0, x1);
            let yy = mth::lerp(step as f64, y0, y1);
            let zz = mth::lerp(step as f64, z0, z1);
            let ss = random.next_double() * size as f64 / 16.0;
            let r = (((mth::sin((std::f64::consts::PI as f32 * step) as f64) + 1.0) as f64) * ss + 1.0) / 2.0;
            data[i * 4] = xx;
            data[i * 4 + 1] = yy;
            data[i * 4 + 2] = zz;
            data[i * 4 + 3] = r;
        }
        for i1 in 0..size.saturating_sub(1) {
            if !(data[i1 * 4 + 3] <= 0.0) {
                for i2 in i1 + 1..size {
                    if !(data[i2 * 4 + 3] <= 0.0) {
                        let dx = data[i1 * 4] - data[i2 * 4];
                        let dy = data[i1 * 4 + 1] - data[i2 * 4 + 1];
                        let dz = data[i1 * 4 + 2] - data[i2 * 4 + 2];
                        let dr = data[i1 * 4 + 3] - data[i2 * 4 + 3];
                        if dr * dr > dx * dx + dy * dy + dz * dz {
                            if dr > 0.0 {
                                data[i2 * 4 + 3] = -1.0;
                            } else {
                                data[i1 * 4 + 3] = -1.0;
                            }
                        }
                    }
                }
            }
        }
        for i in 0..size {
            let r = data[i * 4 + 3];
            if r < 0.0 {
                continue;
            }
            let (xx, yy, zz) = (data[i * 4], data[i * 4 + 1], data[i * 4 + 2]);
            let x_min = mth::floor(xx - r).max(x_start);
            let y_min = mth::floor(yy - r).max(y_start);
            let z_min = mth::floor(zz - r).max(z_start);
            let x_max = mth::floor(xx + r).max(x_min);
            let y_max = mth::floor(yy + r).max(y_min);
            let z_max = mth::floor(zz + r).max(z_min);
            for x in x_min..=x_max {
                let xd = (x as f64 + 0.5 - xx) / r;
                if xd * xd >= 1.0 {
                    continue;
                }
                for y in y_min..=y_max {
                    let yd = (y as f64 + 0.5 - yy) / r;
                    if xd * xd + yd * yd >= 1.0 {
                        continue;
                    }
                    for z in z_min..=z_max {
                        let zd = (z as f64 + 0.5 - zz) / r;
                        if !(xd * xd + yd * yd + zd * zd < 1.0) || level.is_outside_build_height(y) {
                            continue;
                        }
                        let bit_set_index = (x - x_start + (y - y_start) * size_xz + (z - z_start) * size_xz * size_y) as usize;
                        if bit_set_index >= tested.len() {
                            tested.resize(bit_set_index + 1, false);
                        }
                        if tested[bit_set_index] {
                            continue;
                        }
                        tested[bit_set_index] = true;
                        let ore_pos = BlockPos::new(x, y, z);
                        if !level.ensure_can_write(ore_pos) {
                            continue;
                        }
                        let block_state: BlockState = level.get_block_state(ore_pos);
                        for target_state in &self.ore.target_states {
                            if self.ore.can_place_ore(block_state, |pos| section_block(level, pos), random, target_state, ore_pos) {
                                level.set_section_block_state(ore_pos, target_state.state);
                                placed += 1;
                                break;
                            }
                        }
                    }
                }
            }
        }
        placed > 0
    }
}

fn section_block(level: &mut impl WorldGenLevel, pos: BlockPos) -> BlockState {
    if level.is_outside_build_height(pos.y) { blocks::AIR.default_state() } else { level.get_block_state(pos) }
}
