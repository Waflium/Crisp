use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, mth};
use crisp_data::blocks;
use crisp_data::properties::{EAST, NORTH, SOUTH, WEST};
use serde_json::Value;

use crate::world::entity::LevelEntity;
use crate::world::entity::boss::enderdragon::end_crystal::EndCrystal;
use crate::world::level::block::base_fire_block;
use crate::world::level::world_gen_level::WorldGenLevel;

const NUMBER_OF_SPIKES: i32 = 10;
const SPIKE_DISTANCE: f64 = 42.0;

#[derive(Clone, Copy, Debug)]
pub struct EndSpike {
    center_x: i32,
    center_z: i32,
    radius: i32,
    height: i32,
    guarded: bool,
}

impl EndSpike {
    pub fn new(center_x: i32, center_z: i32, radius: i32, height: i32, guarded: bool) -> EndSpike {
        EndSpike { center_x, center_z, radius, height, guarded }
    }

    fn parse(json: &Value) -> EndSpike {
        let int = |key: &str| json.get(key).and_then(Value::as_i64).unwrap_or(0) as i32;
        EndSpike::new(int("centerX"), int("centerZ"), int("radius"), int("height"), json.get("guarded").and_then(Value::as_bool).unwrap_or(false))
    }

    pub fn is_center_within_chunk(&self, chunk_origin: BlockPos) -> bool {
        chunk_origin.x >> 4 == self.center_x >> 4 && chunk_origin.z >> 4 == self.center_z >> 4
    }

    pub fn get_center_x(&self) -> i32 {
        self.center_x
    }

    pub fn get_center_z(&self) -> i32 {
        self.center_z
    }

    pub fn get_radius(&self) -> i32 {
        self.radius
    }

    pub fn get_height(&self) -> i32 {
        self.height
    }

    pub fn is_guarded(&self) -> bool {
        self.guarded
    }
}

pub struct EndSpikeFeature {
    spikes: Vec<EndSpike>,
    crystal_invulnerable: bool,
    crystal_beam_target: Option<BlockPos>,
}

pub fn get_spikes_for_level(seed: i64) -> Vec<EndSpike> {
    let mut random = LegacyRandomSource::new(seed);
    let key = random.next_long() & 65535;
    let mut sizes: Vec<i32> = (0..NUMBER_OF_SPIKES).collect();
    crate::util::shuffle(&mut sizes, &mut LegacyRandomSource::new(key));
    (0..NUMBER_OF_SPIKES as usize)
        .map(|i| {
            let angle = 2.0 * (-std::f64::consts::PI + (std::f64::consts::PI / 10.0) * i as f64);
            let x = mth::floor(SPIKE_DISTANCE * angle.cos());
            let z = mth::floor(SPIKE_DISTANCE * angle.sin());
            let size = sizes[i];
            let radius = 2 + size / 3;
            let height = 76 + size * 3;
            let guarded = size == 1 || size == 2;
            EndSpike::new(x, z, radius, height, guarded)
        })
        .collect()
}

impl EndSpikeFeature {
    pub fn parse(json: &Value) -> Result<EndSpikeFeature, String> {
        let crystal_beam_target = match json.get("crystal_beam_target").and_then(Value::as_array) {
            Some(v) if v.len() == 3 => {
                let coordinate = |i: usize| v[i].as_i64().map(|c| c as i32).ok_or("bad crystal_beam_target");
                Some(BlockPos::new(coordinate(0)?, coordinate(1)?, coordinate(2)?))
            }
            _ => None,
        };
        Ok(EndSpikeFeature {
            spikes: json.get("spikes").and_then(Value::as_array).ok_or("missing spikes")?.iter().map(EndSpike::parse).collect(),
            crystal_invulnerable: json.get("crystal_invulnerable").and_then(Value::as_bool).unwrap_or(false),
            crystal_beam_target,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let spikes = if self.spikes.is_empty() { get_spikes_for_level(level.get_seed()) } else { self.spikes.clone() };
        for spike in &spikes {
            if spike.is_center_within_chunk(origin) {
                self.place_spike(level, random, spike);
            }
        }
        true
    }

    fn place_spike<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, spike: &EndSpike) {
        let radius = spike.get_radius();
        let from = BlockPos::new(spike.get_center_x() - radius, level.get_min_y(), spike.get_center_z() - radius);
        let to = BlockPos::new(spike.get_center_x() + radius, spike.get_height() + 10, spike.get_center_z() + radius);
        for pos in BlockPos::between_closed(from, to) {
            let (dx, dz) = ((pos.x - spike.get_center_x()) as f64, (pos.z - spike.get_center_z()) as f64);
            if dx * dx + dz * dz <= (radius * radius + 1) as f64 && pos.y < spike.get_height() {
                level.set_block_and_update(pos, blocks::OBSIDIAN.default_state());
            } else if pos.y > 65 {
                level.set_block_and_update(pos, blocks::AIR.default_state());
            }
        }
        if spike.is_guarded() {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    for dy in 0..=3 {
                        let is_x_side = dx.abs() == 2;
                        let is_z_side = dz.abs() == 2;
                        let top = dy == 3;
                        if is_x_side || is_z_side || top {
                            let x_edge = dx == -2 || dx == 2 || top;
                            let z_edge = dz == -2 || dz == 2 || top;
                            let state = blocks::IRON_BARS
                                .default_state()
                                .set(NORTH, x_edge && dz != -2)
                                .set(SOUTH, x_edge && dz != 2)
                                .set(WEST, z_edge && dx != -2)
                                .set(EAST, z_edge && dx != 2);
                            let pos = BlockPos::new(spike.get_center_x() + dx, spike.get_height() + dy, spike.get_center_z() + dz);
                            level.set_block_and_update(pos, state);
                        }
                    }
                }
            }
        }
        let mut end_crystal = EndCrystal::of_type();
        end_crystal.set_beam_target(self.crystal_beam_target);
        end_crystal.entity.invulnerable = self.crystal_invulnerable;
        let y_rot = random.next_float() * 360.0;
        end_crystal.entity.snap_to(spike.get_center_x() as f64 + 0.5, (spike.get_height() + 1) as f64, spike.get_center_z() as f64 + 0.5, y_rot, 0.0);
        let crystal_pos = end_crystal.entity.block_position();
        level.add_fresh_entity(LevelEntity::EndCrystal(end_crystal));
        level.set_block_and_update(crystal_pos.below(), blocks::BEDROCK.default_state());
        let fire = base_fire_block::get_state(level, crystal_pos);
        level.set_block_and_update(crystal_pos, fire);
    }
}
