use std::sync::{Arc, LazyLock};

use crisp_core::{ChunkPos, mth};

use super::density_function::DensityVolume;
use super::structure::BoundingBox;
use super::structure::structure_start::StructureStart;
use super::structure::terrain_adjustment::TerrainAdjustment;

const BEARD_KERNEL_SIZE: i32 = 24;

static BEARD_KERNEL: LazyLock<Vec<f32>> = LazyLock::new(|| {
    let mut kernel = vec![0.0; (BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE) as usize];
    for zi in 0..BEARD_KERNEL_SIZE {
        for xi in 0..BEARD_KERNEL_SIZE {
            for yi in 0..BEARD_KERNEL_SIZE {
                kernel[(zi * 24 * 24 + xi * 24 + yi) as usize] = compute_beard_contribution(xi - 12, yi - 12, zi - 12) as f32;
            }
        }
    }
    kernel
});

pub struct Rigid {
    pub bounding_box: BoundingBox,
    pub terrain_adjustment: TerrainAdjustment,
    pub ground_level_delta: i32,
}

pub struct JigsawJunction {
    pub source_x: i32,
    pub source_ground_y: i32,
    pub source_z: i32,
}

pub struct Beardifier {
    pieces: Vec<Rigid>,
    junctions: Vec<JigsawJunction>,
    affected_box: Option<BoundingBox>,
}

fn include_bounding_box(encompassing_box: Option<BoundingBox>, new_box: &BoundingBox) -> Option<BoundingBox> {
    Some(match encompassing_box {
        None => *new_box,
        Some(encompassing_box) => BoundingBox::encapsulating(&encompassing_box, new_box),
    })
}

impl Beardifier {
    pub fn for_structures_in_chunk(structure_starts: &[Arc<StructureStart>], chunk_pos: ChunkPos) -> Option<Beardifier> {
        if structure_starts.is_empty() {
            return None;
        }
        let mut rigids = Vec::new();
        let junctions = Vec::new();
        let mut any_piece_bounding_box = None;
        for start in structure_starts {
            let terrain_adjustment = start.structure.value().terrain_adaptation();
            let container = start.piece_container.lock().unwrap();
            for piece in &container.pieces {
                let piece = piece.piece();
                if piece.is_close_to_chunk(chunk_pos, 12) {
                    rigids.push(Rigid { bounding_box: piece.bounding_box, terrain_adjustment, ground_level_delta: 0 });
                    any_piece_bounding_box = include_bounding_box(any_piece_bounding_box, &piece.bounding_box);
                }
            }
        }
        let affected_box = any_piece_bounding_box?.inflated_by(24);
        Some(Beardifier { pieces: rigids, junctions, affected_box: Some(affected_box) })
    }

    pub fn sample_volume(&self, out: &mut [f32], volume: &DensityVolume) {
        out.fill(0.0);
        let Some(affected_box) = self.affected_box else { return };
        if !affected_box.intersects(&BoundingBox::new(volume.min_x, volume.min_y, volume.min_z, volume.max_x(), volume.max_y(), volume.max_z())) {
            return;
        }
        let min_x = (affected_box.min.x - volume.min_x).max(0).div_euclid(volume.step_x);
        let min_y = (affected_box.min.y - volume.min_y).max(0).div_euclid(volume.step_y);
        let min_z = (affected_box.min.z - volume.min_z).max(0).div_euclid(volume.step_z);
        let max_x = (volume.size_x - 1).min((affected_box.max.x - volume.min_x).div_euclid(volume.step_x));
        let max_y = (volume.size_y - 1).min((affected_box.max.y - volume.min_y).div_euclid(volume.step_y));
        let max_z = (volume.size_z - 1).min((affected_box.max.z - volume.min_z).div_euclid(volume.step_z));
        for z in min_z..=max_z {
            let block_z = volume.block_z(z);
            for x in min_x..=max_x {
                let block_x = volume.block_x(x);
                for y in min_y..=max_y {
                    let index = volume.index(x, y, z);
                    let block_y = volume.block_y(y);
                    out[index] = self.sample_value_unchecked(block_x, block_y, block_z);
                }
            }
        }
    }

    pub fn sample_value(&self, block_x: i32, block_y: i32, block_z: i32) -> f32 {
        match self.affected_box {
            Some(affected_box) if affected_box.is_inside(crisp_core::BlockPos::new(block_x, block_y, block_z)) => self.sample_value_unchecked(block_x, block_y, block_z),
            _ => 0.0,
        }
    }

    fn sample_value_unchecked(&self, block_x: i32, block_y: i32, block_z: i32) -> f32 {
        let mut noise_value = 0.0f32;
        for rigid in &self.pieces {
            let bounding_box = &rigid.bounding_box;
            let ground_level_delta = rigid.ground_level_delta;
            let dx = 0.max((bounding_box.min.x - block_x).max(block_x - bounding_box.max.x));
            let dz = 0.max((bounding_box.min.z - block_z).max(block_z - bounding_box.max.z));
            let ground_y = bounding_box.min.y + ground_level_delta;
            let dy_to_ground = block_y - ground_y;
            let dy = match rigid.terrain_adjustment {
                TerrainAdjustment::None => 0,
                TerrainAdjustment::Bury | TerrainAdjustment::BeardThin => dy_to_ground,
                TerrainAdjustment::BeardBox => 0.max((ground_y - block_y).max(block_y - bounding_box.max.y)),
                TerrainAdjustment::Encapsulate => 0.max((bounding_box.min.y - block_y).max(block_y - bounding_box.max.y)),
            };
            noise_value += match rigid.terrain_adjustment {
                TerrainAdjustment::None => 0.0,
                TerrainAdjustment::Bury => get_bury_contribution(dx as f32, dy as f32 / 2.0, dz as f32),
                TerrainAdjustment::BeardThin | TerrainAdjustment::BeardBox => get_beard_contribution(dx, dy, dz, dy_to_ground) * 0.8,
                TerrainAdjustment::Encapsulate => get_bury_contribution(dx as f32 / 2.0, dy as f32 / 2.0, dz as f32 / 2.0) * 0.8,
            };
        }
        for junction in &self.junctions {
            let dx = block_x - junction.source_x;
            let dy = block_y - junction.source_ground_y;
            let dz = block_z - junction.source_z;
            noise_value += get_beard_contribution(dx, dy, dz, dy) * 0.4;
        }
        noise_value
    }
}

fn get_bury_contribution(dx: f32, dy: f32, dz: f32) -> f32 {
    let distance_sq = dx * dx + dy * dy + dz * dz;
    if distance_sq >= 36.0 { 0.0 } else { 1.0 - distance_sq.sqrt() / 6.0 }
}

fn get_beard_contribution(dx: i32, dy: i32, dz: i32, y_to_ground: i32) -> f32 {
    let xi = dx + 12;
    let yi = dy + 12;
    let zi = dz + 12;
    if !is_in_kernel_range(xi) || !is_in_kernel_range(yi) || !is_in_kernel_range(zi) {
        return 0.0;
    }
    let dy_with_offset = y_to_ground as f32 + 0.5;
    let distance_sqr = dx as f32 * dx as f32 + dy_with_offset * dy_with_offset + dz as f32 * dz as f32;
    let value = -dy_with_offset * mth::fast_inv_sqrt((distance_sqr / 2.0) as f64) as f32 / 2.0;
    value * BEARD_KERNEL[(zi * 24 * 24 + xi * 24 + yi) as usize]
}

fn is_in_kernel_range(xi: i32) -> bool {
    (0..24).contains(&xi)
}

fn compute_beard_contribution(dx: i32, dy: i32, dz: i32) -> f64 {
    let dy = dy as f64 + 0.5;
    let distance_sqr = dx as f64 * dx as f64 + dy * dy + dz as f64 * dz as f64;
    std::f64::consts::E.powf(-distance_sqr / 16.0)
}
