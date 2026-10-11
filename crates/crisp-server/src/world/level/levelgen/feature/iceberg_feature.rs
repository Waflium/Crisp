use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::parse_block_state;

pub struct IcebergFeature {
    state: BlockState,
}

fn pow2(value: f64) -> f64 {
    value * value
}

impl IcebergFeature {
    pub fn parse(json: &Value) -> Result<IcebergFeature, String> {
        Ok(IcebergFeature { state: parse_block_state(json.get("state").ok_or("missing state")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let origin = BlockPos::new(origin.x, level.get_sea_level(), origin.z);
        let snow_on_top = random.next_double() > 0.7;
        let main_block_state = self.state;
        let shape_angle = random.next_double() * 2.0 * std::f64::consts::PI;
        let shape_ellipse_a = 11 - random.next_int_bounded(5);
        let shape_ellipse_c = 3 + random.next_int_bounded(3);
        let is_ellipse = random.next_double() > 0.7;
        let mut over_water_height = if is_ellipse { random.next_int_bounded(6) + 6 } else { random.next_int_bounded(15) + 3 };
        if !is_ellipse && random.next_double() > 0.9 {
            over_water_height += random.next_int_bounded(19) + 7;
        }
        let under_water_height = (over_water_height + random.next_int_bounded(11)).min(18);
        let width = (over_water_height + random.next_int_bounded(7) - random.next_int_bounded(5)).min(11);
        let a = if is_ellipse { shape_ellipse_a } else { 11 };
        for xo in -a..a {
            for zo in -a..a {
                for y_off in 0..over_water_height {
                    let radius = if is_ellipse {
                        height_dependent_radius_ellipse(y_off, over_water_height, width)
                    } else {
                        height_dependent_radius_round(random, y_off, over_water_height, width)
                    };
                    if is_ellipse || xo < radius {
                        generate_iceberg_block(level, random, origin, over_water_height, xo, y_off, zo, radius, a, is_ellipse, shape_ellipse_c, shape_angle, snow_on_top, main_block_state);
                    }
                }
            }
        }
        smooth(level, origin, width, over_water_height, is_ellipse, shape_ellipse_a);
        for xo in -a..a {
            for zo in -a..a {
                let mut y_off = -1;
                while y_off > -under_water_height {
                    let new_a = if is_ellipse {
                        mth::ceil((a as f32 * (1.0 - pow2(y_off as f64) as f32 / (under_water_height as f32 * 8.0))) as f64)
                    } else {
                        a
                    };
                    let radius = height_dependent_radius_steep(random, -y_off, under_water_height, width);
                    if xo < radius {
                        generate_iceberg_block(level, random, origin, under_water_height, xo, y_off, zo, radius, new_a, is_ellipse, shape_ellipse_c, shape_angle, snow_on_top, main_block_state);
                    }
                    y_off -= 1;
                }
            }
        }
        let do_cut_out = if is_ellipse { random.next_double() > 0.1 } else { random.next_double() > 0.7 };
        if do_cut_out {
            generate_cut_out(random, level, width, over_water_height, origin, is_ellipse, shape_ellipse_a, shape_angle, shape_ellipse_c);
        }
        true
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_cut_out<R: RandomSource>(
    random: &mut R,
    level: &mut WorldGenRegion,
    width: i32,
    height: i32,
    global_origin: BlockPos,
    is_ellipse: bool,
    shape_ellipse_a: i32,
    shape_angle: f64,
    shape_ellipse_c: i32,
) {
    let random_sign_x = if random.next_boolean() { -1 } else { 1 };
    let random_sign_z = if random.next_boolean() { -1 } else { 1 };
    let mut x_off = random.next_int_bounded((width / 2 - 2).max(1));
    if random.next_boolean() {
        x_off = width / 2 + 1 - random.next_int_bounded((width - width / 2 - 1).max(1));
    }
    let mut z_off = random.next_int_bounded((width / 2 - 2).max(1));
    if random.next_boolean() {
        z_off = width / 2 + 1 - random.next_int_bounded((width - width / 2 - 1).max(1));
    }
    if is_ellipse {
        x_off = random.next_int_bounded((shape_ellipse_a - 5).max(1));
        z_off = x_off;
    }
    let local_origin = BlockPos::new(random_sign_x * x_off, 0, random_sign_z * z_off);
    let angle = if is_ellipse { shape_angle + std::f64::consts::PI / 2.0 } else { random.next_double() * 2.0 * std::f64::consts::PI };
    for y_off in 0..height - 3 {
        let radius = height_dependent_radius_round(random, y_off, height, width);
        carve(radius, y_off, global_origin, level, false, angle, local_origin, shape_ellipse_a, shape_ellipse_c);
    }
    let mut y_off = -1;
    while y_off > -height + random.next_int_bounded(5) {
        let radius = height_dependent_radius_steep(random, -y_off, height, width);
        carve(radius, y_off, global_origin, level, true, angle, local_origin, shape_ellipse_a, shape_ellipse_c);
        y_off -= 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn carve(radius: i32, y_off: i32, global_origin: BlockPos, level: &mut WorldGenRegion, under_water: bool, angle: f64, local_origin: BlockPos, shape_ellipse_a: i32, shape_ellipse_c: i32) {
    let a = radius + 1 + shape_ellipse_a / 3;
    let c = (radius - 3).min(3) + shape_ellipse_c / 2 - 1;
    for xo in -a..a {
        for zo in -a..a {
            let signed_dist = signed_distance_ellipse(xo, zo, local_origin, a, c, angle);
            if signed_dist < 0.0 {
                let pos = global_origin.offset(xo, y_off, zo);
                let state = level.get_block_state(pos);
                if is_iceberg_state(state) || state.is_block(blocks::SNOW_BLOCK) {
                    if under_water {
                        level.set_block_and_update(pos, blocks::WATER.default_state());
                    } else {
                        level.set_block_and_update(pos, blocks::AIR.default_state());
                        remove_floating_snow_layer(level, pos);
                    }
                }
            }
        }
    }
}

fn remove_floating_snow_layer(level: &mut WorldGenRegion, pos: BlockPos) {
    if level.get_block_state(pos.above()).is_block(blocks::SNOW) {
        level.set_block_and_update(pos.above(), blocks::AIR.default_state());
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_iceberg_block<R: RandomSource>(
    level: &mut WorldGenRegion,
    random: &mut R,
    origin: BlockPos,
    height: i32,
    xo: i32,
    y_off: i32,
    zo: i32,
    radius: i32,
    a: i32,
    is_ellipse: bool,
    shape_ellipse_c: i32,
    shape_angle: f64,
    snow_on_top: bool,
    main_block_state: BlockState,
) {
    let signed_dist = if is_ellipse {
        signed_distance_ellipse(xo, zo, BlockPos::new(0, 0, 0), a, get_ellipse_c(y_off, height, shape_ellipse_c), shape_angle)
    } else {
        signed_distance_circle(xo, zo, BlockPos::new(0, 0, 0), radius, random)
    };
    if signed_dist < 0.0 {
        let pos = origin.offset(xo, y_off, zo);
        let compare_val = if is_ellipse { -0.5 } else { (-6 - random.next_int_bounded(3)) as f64 };
        if signed_dist > compare_val && random.next_double() > 0.9 {
            return;
        }
        set_iceberg_block(pos, level, random, height - y_off, height, is_ellipse, snow_on_top, main_block_state);
    }
}

#[allow(clippy::too_many_arguments)]
fn set_iceberg_block<R: RandomSource>(pos: BlockPos, level: &mut WorldGenRegion, random: &mut R, h_diff: i32, height: i32, is_ellipse: bool, snow_on_top: bool, main_block_state: BlockState) {
    let state = level.get_block_state(pos);
    if state.is_air() || state.is_block(blocks::SNOW_BLOCK) || state.is_block(blocks::ICE) || state.is_block(blocks::WATER) {
        let randomness = !is_ellipse || random.next_double() > 0.05;
        let divisor = if is_ellipse { 3 } else { 2 };
        if snow_on_top && !state.is_block(blocks::WATER) && (h_diff as f64) <= (random.next_int_bounded(1.max(height / divisor)) as f64) + height as f64 * 0.6 && randomness {
            level.set_block_and_update(pos, blocks::SNOW_BLOCK.default_state());
        } else {
            level.set_block_and_update(pos, main_block_state);
        }
    }
}

fn get_ellipse_c(y_off: i32, height: i32, shape_ellipse_c: i32) -> i32 {
    let mut c = shape_ellipse_c;
    if y_off > 0 && height - y_off <= 3 {
        c -= 4 - (height - y_off);
    }
    c
}

fn signed_distance_circle(xo: i32, zo: i32, origin: BlockPos, radius: i32, random: &mut impl RandomSource) -> f64 {
    let off = 10.0 * random.next_float().clamp(0.2, 0.8) / radius as f32;
    off as f64 + pow2((xo - origin.x) as f64) + pow2((zo - origin.z) as f64) - pow2(radius as f64)
}

fn signed_distance_ellipse(xo: i32, zo: i32, origin: BlockPos, a: i32, c: i32, angle: f64) -> f64 {
    let dx = (xo - origin.x) as f64;
    let dz = (zo - origin.z) as f64;
    pow2((dx * angle.cos() - dz * angle.sin()) / a as f64) + pow2((dx * angle.sin() + dz * angle.cos()) / c as f64) - 1.0
}

fn height_dependent_radius_round(random: &mut impl RandomSource, y_off: i32, height: i32, width: i32) -> i32 {
    let k = 3.5 - random.next_float();
    let mut scale = (1.0 - pow2(y_off as f64) as f32 / (height as f32 * k)) * width as f32;
    if height > 15 + random.next_int_bounded(5) {
        let temp_y_off = if y_off < 3 + random.next_int_bounded(6) { y_off / 2 } else { y_off };
        scale = (1.0 - temp_y_off as f32 / (height as f32 * k * 0.4)) * width as f32;
    }
    mth::ceil((scale / 2.0) as f64)
}

fn height_dependent_radius_ellipse(y_off: i32, height: i32, width: i32) -> i32 {
    let scale = (1.0 - pow2(y_off as f64) as f32 / (height as f32 * 1.0)) * width as f32;
    mth::ceil((scale / 2.0) as f64)
}

fn height_dependent_radius_steep(random: &mut impl RandomSource, y_off: i32, height: i32, width: i32) -> i32 {
    let k = 1.0 + random.next_float() / 2.0;
    let scale = (1.0 - y_off as f32 / (height as f32 * k)) * width as f32;
    mth::ceil((scale / 2.0) as f64)
}

fn is_iceberg_state(state: BlockState) -> bool {
    state.is_block(blocks::PACKED_ICE) || state.is_block(blocks::SNOW_BLOCK) || state.is_block(blocks::BLUE_ICE)
}

fn below_is_air(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    level.get_block_state(pos.below()).is_air()
}

fn smooth(level: &mut WorldGenRegion, origin: BlockPos, width: i32, height: i32, is_ellipse: bool, shape_ellipse_a: i32) {
    let a = if is_ellipse { shape_ellipse_a } else { width / 2 };
    for x in -a..=a {
        for z in -a..=a {
            for y_off in 0..=height {
                let pos = origin.offset(x, y_off, z);
                let state = level.get_block_state(pos);
                if is_iceberg_state(state) || state.is_block(blocks::SNOW) {
                    if below_is_air(level, pos) {
                        level.set_block_and_update(pos, blocks::AIR.default_state());
                        level.set_block_and_update(pos.above(), blocks::AIR.default_state());
                    } else if is_iceberg_state(state) {
                        let sides = [level.get_block_state(pos.west()), level.get_block_state(pos.east()), level.get_block_state(pos.north()), level.get_block_state(pos.south())];
                        let counter = sides.iter().filter(|&&side| !is_iceberg_state(side)).count();
                        if counter >= 3 {
                            level.set_block_and_update(pos, blocks::AIR.default_state());
                        }
                    }
                }
            }
        }
    }
}
