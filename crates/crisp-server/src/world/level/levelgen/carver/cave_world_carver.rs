use crisp_core::{ChunkPos, RandomSource, mth};
use serde_json::Value;

use super::{WorldGenerationContext, can_reach, carve_ellipsoid, tunnel_random};
use crate::util::valueproviders::{FloatProvider, IntProvider, float};
use crate::world::level::chunk::carving_mask::CarvingMask;
use crate::world::level::levelgen::heightproviders::HeightProvider;

pub struct CaveWorldCarver {
    pub probability: f32,
    y: HeightProvider,
    count: IntProvider,
    thickness: FloatProvider,
    weird_thickness_bias: bool,
    room_vertical_radius_multiplier: FloatProvider,
    horizontal_radius_multiplier: FloatProvider,
    vertical_radius_multiplier: FloatProvider,
    start_vertical_radius_multiplier: FloatProvider,
    floor_level: FloatProvider,
}

struct Tunnel {
    horizontal_radius_multiplier: f64,
    vertical_radius_multiplier: f64,
    floor_level: f64,
}

impl CaveWorldCarver {
    pub fn parse(json: &Value) -> Result<CaveWorldCarver, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let float_provider = |key: &str| FloatProvider::parse(field(key)?);
        Ok(CaveWorldCarver {
            probability: float(field("probability")?)?,
            y: HeightProvider::parse(field("y")?)?,
            count: IntProvider::parse(field("count")?)?,
            thickness: float_provider("thickness")?,
            weird_thickness_bias: json.get("weird_thickness_bias").and_then(Value::as_bool).unwrap_or(false),
            room_vertical_radius_multiplier: float_provider("room_vertical_radius_multiplier")?,
            horizontal_radius_multiplier: float_provider("horizontal_radius_multiplier")?,
            vertical_radius_multiplier: float_provider("vertical_radius_multiplier")?,
            start_vertical_radius_multiplier: match json.get("start_vertical_radius_multiplier") {
                Some(v) => FloatProvider::parse(v)?,
                None => FloatProvider::Constant(1.0),
            },
            floor_level: float_provider("floor_level")?,
        })
    }

    pub fn carve(
        &self,
        range: i32,
        context: &WorldGenerationContext,
        random: &mut impl RandomSource,
        chunk: ChunkPos,
        source: ChunkPos,
        mask: &mut CarvingMask,
    ) {
        let max_distance = (range * 2 - 1) << 4;
        let cave_count = self.count.sample(random);
        for _ in 0..cave_count {
            let x = (source.min_block_x() + random.next_int_bounded(16)) as f64;
            let y = self.y.sample(random, context) as f64;
            let z = (source.min_block_z() + random.next_int_bounded(16)) as f64;
            let tunnel = Tunnel {
                horizontal_radius_multiplier: self.horizontal_radius_multiplier.sample(random) as f64,
                vertical_radius_multiplier: self.vertical_radius_multiplier.sample(random) as f64,
                floor_level: 0.0,
            };
            let start_vertical_radius_multiplier = self.start_vertical_radius_multiplier.sample(random) as f64;
            let tunnel = Tunnel { floor_level: self.floor_level.sample(random) as f64, ..tunnel };
            let mut tunnels = 1;
            if random.next_int_bounded(4) == 0 {
                let y_scale = self.room_vertical_radius_multiplier.sample(random) as f64;
                let thickness = 1.0 + random.next_float() * 6.0;
                create_room(chunk, x, y, z, thickness, y_scale, mask, tunnel.floor_level);
                tunnels += random.next_int_bounded(4);
            }
            for _ in 0..tunnels {
                let horizontal_rotation = random.next_float() * std::f64::consts::TAU as f32;
                let vertical_rotation = (random.next_float() - 0.5) / 4.0;
                let thickness = self.thickness(random);
                let distance = max_distance - random.next_int_bounded(max_distance / 4);
                let seed = random.next_long();
                create_tunnel(
                    chunk,
                    seed,
                    x,
                    y,
                    z,
                    &tunnel,
                    thickness,
                    horizontal_rotation,
                    vertical_rotation,
                    0,
                    distance,
                    start_vertical_radius_multiplier,
                    mask,
                );
            }
        }
    }

    fn thickness(&self, random: &mut impl RandomSource) -> f32 {
        let mut thickness = self.thickness.sample(random);
        if self.weird_thickness_bias && random.next_int_bounded(10) == 0 {
            thickness *= random.next_float() * random.next_float() * 3.0 + 1.0;
        }
        thickness
    }
}

fn should_skip(xd: f64, yd: f64, zd: f64, floor_level: f64) -> bool {
    if yd <= floor_level { true } else { xd * xd + yd * yd + zd * zd >= 1.0 }
}

#[allow(clippy::too_many_arguments)]
fn create_room(chunk: ChunkPos, x: f64, y: f64, z: f64, thickness: f32, y_scale: f64, mask: &mut CarvingMask, floor_level: f64) {
    let horizontal_radius = 1.5 + (mth::sin(std::f64::consts::FRAC_PI_2 as f32 as f64) * thickness) as f64;
    let vertical_radius = horizontal_radius * y_scale;
    carve_ellipsoid(chunk, x + 1.0, y, z, horizontal_radius, vertical_radius, mask, |xd, yd, zd, _| should_skip(xd, yd, zd, floor_level));
}

#[allow(clippy::too_many_arguments)]
fn create_tunnel(
    chunk: ChunkPos,
    tunnel_seed: i64,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    tunnel: &Tunnel,
    thickness: f32,
    mut horizontal_rotation: f32,
    mut vertical_rotation: f32,
    step: i32,
    dist: i32,
    y_scale: f64,
    mask: &mut CarvingMask,
) {
    let mut random = tunnel_random(tunnel_seed);
    let split_point = random.next_int_bounded(dist / 2) + dist / 4;
    let steep = random.next_int_bounded(6) == 0;
    let mut y_rota = 0.0f32;
    let mut x_rota = 0.0f32;
    for current_step in step..dist {
        let horizontal_radius = 1.5 + (mth::sin((std::f32::consts::PI * current_step as f32 / dist as f32) as f64) * thickness) as f64;
        let vertical_radius = horizontal_radius * y_scale;
        let cos_x = mth::cos(vertical_rotation as f64);
        x += (mth::cos(horizontal_rotation as f64) * cos_x) as f64;
        y += mth::sin(vertical_rotation as f64) as f64;
        z += (mth::sin(horizontal_rotation as f64) * cos_x) as f64;
        vertical_rotation *= if steep { 0.92 } else { 0.7 };
        vertical_rotation += x_rota * 0.1;
        horizontal_rotation += y_rota * 0.1;
        x_rota *= 0.9;
        y_rota *= 0.75;
        x_rota += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
        y_rota += (random.next_float() - random.next_float()) * random.next_float() * 4.0;
        if current_step == split_point && thickness > 1.0 {
            let half_pi = std::f64::consts::FRAC_PI_2 as f32;
            let seed = random.next_long();
            let branch_thickness = random.next_float() * 0.5 + 0.5;
            create_tunnel(
                chunk,
                seed,
                x,
                y,
                z,
                tunnel,
                branch_thickness,
                horizontal_rotation - half_pi,
                vertical_rotation / 3.0,
                current_step,
                dist,
                1.0,
                mask,
            );
            let seed = random.next_long();
            let branch_thickness = random.next_float() * 0.5 + 0.5;
            create_tunnel(
                chunk,
                seed,
                x,
                y,
                z,
                tunnel,
                branch_thickness,
                horizontal_rotation + half_pi,
                vertical_rotation / 3.0,
                current_step,
                dist,
                1.0,
                mask,
            );
            return;
        }
        if random.next_int_bounded(4) != 0 {
            if !can_reach(chunk, x, z, current_step, dist, thickness) {
                return;
            }
            let floor_level = tunnel.floor_level;
            carve_ellipsoid(
                chunk,
                x,
                y,
                z,
                horizontal_radius * tunnel.horizontal_radius_multiplier,
                vertical_radius * tunnel.vertical_radius_multiplier,
                mask,
                |xd, yd, zd, _| should_skip(xd, yd, zd, floor_level),
            );
        }
    }
}
