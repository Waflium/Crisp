use crisp_core::{ChunkPos, RandomSource, mth};
use serde_json::Value;

use super::{WorldGenerationContext, can_reach, carve_ellipsoid, tunnel_random};
use crate::util::valueproviders::{FloatProvider, float, random_between};
use crate::world::level::chunk::carving_mask::CarvingMask;
use crate::world::level::levelgen::heightproviders::HeightProvider;

pub struct CanyonWorldCarver {
    pub probability: f32,
    y: HeightProvider,
    vertical_rotation: FloatProvider,
    shape: Shape,
}

struct Shape {
    distance_factor: FloatProvider,
    thickness: FloatProvider,
    width_smoothness: i32,
    horizontal_radius_factor: FloatProvider,
    vertical_radius_default_factor: f32,
    vertical_radius_center_factor: f32,
    y_scale: FloatProvider,
}

impl CanyonWorldCarver {
    pub fn parse(json: &Value) -> Result<CanyonWorldCarver, String> {
        let field = |v: &Value, key: &str| v.get(key).cloned().ok_or_else(|| format!("missing {key}"));
        let shape = field(json, "shape")?;
        let provider = |key: &str| FloatProvider::parse(&field(&shape, key)?);
        Ok(CanyonWorldCarver {
            probability: float(&field(json, "probability")?)?,
            y: HeightProvider::parse(&field(json, "y")?)?,
            vertical_rotation: FloatProvider::parse(&field(json, "vertical_rotation")?)?,
            shape: Shape {
                distance_factor: provider("distance_factor")?,
                thickness: provider("thickness")?,
                width_smoothness: field(&shape, "width_smoothness")?.as_i64().ok_or("width_smoothness")? as i32,
                horizontal_radius_factor: provider("horizontal_radius_factor")?,
                vertical_radius_default_factor: float(&field(&shape, "vertical_radius_default_factor")?)?,
                vertical_radius_center_factor: float(&field(&shape, "vertical_radius_center_factor")?)?,
                y_scale: provider("y_scale")?,
            },
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
        let max_distance = (range * 2 - 1) * 16;
        let x = (source.min_block_x() + random.next_int_bounded(16)) as f64;
        let y = self.y.sample(random, context);
        let z = (source.min_block_z() + random.next_int_bounded(16)) as f64;
        let horizontal_rotation = random.next_float() * std::f64::consts::TAU as f32;
        let vertical_rotation = self.vertical_rotation.sample(random);
        let y_scale = self.shape.y_scale.sample(random) as f64;
        let thickness = self.shape.thickness.sample(random);
        let distance = (max_distance as f32 * self.shape.distance_factor.sample(random)) as i32;
        let seed = random.next_long();
        self.do_carve(context, chunk, seed, x, y as f64, z, thickness, horizontal_rotation, vertical_rotation, 0, distance, y_scale, mask);
    }

    #[allow(clippy::too_many_arguments)]
    fn do_carve(
        &self,
        context: &WorldGenerationContext,
        chunk: ChunkPos,
        tunnel_seed: i64,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        thickness: f32,
        mut horizontal_rotation: f32,
        mut vertical_rotation: f32,
        step: i32,
        distance: i32,
        y_scale: f64,
        mask: &mut CarvingMask,
    ) {
        let mut random = tunnel_random(tunnel_seed);
        let width_factor_per_height = self.init_width_factors(context, &mut random);
        let mut y_rota = 0.0f32;
        let mut x_rota = 0.0f32;
        for current_step in step..distance {
            let mut horizontal_radius = 1.5 + (mth::sin((current_step as f32 * std::f32::consts::PI / distance as f32) as f64) * thickness) as f64;
            let mut vertical_radius = horizontal_radius * y_scale;
            horizontal_radius *= self.shape.horizontal_radius_factor.sample(&mut random) as f64;
            vertical_radius = self.update_vertical_radius(&mut random, vertical_radius, distance as f32, current_step as f32);
            let xc = mth::cos(vertical_rotation as f64);
            let xs = mth::sin(vertical_rotation as f64);
            x += (mth::cos(horizontal_rotation as f64) * xc) as f64;
            y += xs as f64;
            z += (mth::sin(horizontal_rotation as f64) * xc) as f64;
            vertical_rotation *= 0.7;
            vertical_rotation += x_rota * 0.05;
            horizontal_rotation += y_rota * 0.05;
            x_rota *= 0.8;
            y_rota *= 0.5;
            x_rota += (random.next_float() - random.next_float()) * random.next_float() * 2.0;
            y_rota += (random.next_float() - random.next_float()) * random.next_float() * 4.0;
            if random.next_int_bounded(4) != 0 {
                if !can_reach(chunk, x, z, current_step, distance, thickness) {
                    return;
                }
                let min_gen_y = context.min_gen_y;
                carve_ellipsoid(chunk, x, y, z, horizontal_radius, vertical_radius, mask, |xd, yd, zd, world_y| {
                    let y_index = world_y - min_gen_y;
                    (xd * xd + zd * zd) * width_factor_per_height[(y_index - 1) as usize] as f64 + yd * yd / 6.0 >= 1.0
                });
            }
        }
    }

    fn init_width_factors(&self, context: &WorldGenerationContext, random: &mut impl RandomSource) -> Vec<f32> {
        let depth = context.gen_depth;
        let mut width_factor_per_height = vec![0.0f32; depth as usize];
        let mut width_factor = 1.0f32;
        for (y_index, factor) in width_factor_per_height.iter_mut().enumerate() {
            if y_index == 0 || random.next_int_bounded(self.shape.width_smoothness) == 0 {
                width_factor = 1.0 + random.next_float() * random.next_float();
            }
            *factor = width_factor * width_factor;
        }
        width_factor_per_height
    }

    fn update_vertical_radius(&self, random: &mut impl RandomSource, vertical_radius: f64, distance: f32, current_step: f32) -> f64 {
        let vertical_multiplier = 1.0 - (0.5 - current_step / distance).abs() * 2.0;
        let factor = self.shape.vertical_radius_default_factor + self.shape.vertical_radius_center_factor * vertical_multiplier;
        factor as f64 * vertical_radius * random_between(random, 0.75, 1.0) as f64
    }
}
