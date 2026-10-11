use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::java_collections::JavaObjectHashSet;
use crate::util::valueproviders::{IntProvider, float};
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct VegetationPatchFeature {
    replaceable: BlockSet,
    ground_state: BlockStateProviderHolder,
    vegetation_feature: PlacedFeatureHolder,
    surface: Direction,
    depth: IntProvider,
    extra_bottom_block_chance: f32,
    vertical_range: i32,
    vegetation_chance: f32,
    xz_radius: IntProvider,
    extra_edge_column_chance: f32,
    waterlogged: bool,
}

impl VegetationPatchFeature {
    pub fn parse(json: &Value, waterlogged: bool) -> Result<VegetationPatchFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let surface = match field("surface")?.as_str() {
            Some("floor") => Direction::Down,
            Some("ceiling") => Direction::Up,
            other => return Err(format!("unknown cave surface {other:?}")),
        };
        Ok(VegetationPatchFeature {
            replaceable: BlockSet::parse(field("replaceable")?)?,
            ground_state: BlockStateProviderHolder::parse(field("ground_state")?)?,
            vegetation_feature: PlacedFeatureHolder::parse(field("vegetation_feature")?)?,
            surface,
            depth: IntProvider::parse(field("depth")?)?,
            extra_bottom_block_chance: float(field("extra_bottom_block_chance")?)?,
            vertical_range: field("vertical_range")?.as_i64().ok_or("bad vertical_range")? as i32,
            vegetation_chance: float(field("vegetation_chance")?)?,
            xz_radius: IntProvider::parse(field("xz_radius")?)?,
            extra_edge_column_chance: float(field("extra_edge_column_chance")?)?,
            waterlogged,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let x_radius = self.xz_radius.sample(random) + 1;
        let z_radius = self.xz_radius.sample(random) + 1;
        let surface = if self.waterlogged {
            self.waterlogged_place_ground_patch(level, random, origin, x_radius, z_radius)
        } else {
            self.place_ground_patch(level, random, origin, x_radius, z_radius)
        };
        self.distribute_vegetation(level, random, &surface);
        !surface.is_empty()
    }

    fn place_ground_patch<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos, x_radius: i32, z_radius: i32) -> JavaObjectHashSet<BlockPos> {
        let inwards = self.surface;
        let outwards = inwards.opposite();
        let mut surface = JavaObjectHashSet::default();
        for dx in -x_radius..=x_radius {
            let is_x_edge = dx == -x_radius || dx == x_radius;
            for dz in -z_radius..=z_radius {
                let is_z_edge = dz == -z_radius || dz == z_radius;
                let is_edge = is_x_edge || is_z_edge;
                let is_corner = is_x_edge && is_z_edge;
                let is_edge_but_not_corner = is_edge && !is_corner;
                if !is_corner && (!is_edge_but_not_corner || self.extra_edge_column_chance != 0.0 && random.next_float() <= self.extra_edge_column_chance) {
                    let mut pos = origin.offset(dx, 0, dz);
                    let mut offset = 0;
                    while level.get_block_state(pos).is_air() && offset < self.vertical_range {
                        pos = pos.relative(inwards);
                        offset += 1;
                    }
                    let mut offset = 0;
                    while !level.get_block_state(pos).is_air() && offset < self.vertical_range {
                        pos = pos.relative(outwards);
                        offset += 1;
                    }
                    let below_pos = pos.relative(self.surface);
                    let below_state = level.get_block_state(below_pos);
                    if level.get_block_state(pos).is_air() && below_state.is_face_sturdy_full(self.surface.opposite()) {
                        let depth = self.depth.sample(random) + if self.extra_bottom_block_chance > 0.0 && random.next_float() < self.extra_bottom_block_chance { 1 } else { 0 };
                        let ground_pos = below_pos;
                        if self.place_ground(level, random, below_pos, depth) {
                            surface.insert(ground_pos);
                        }
                    }
                }
            }
        }
        surface
    }

    fn waterlogged_place_ground_patch<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos, x_radius: i32, z_radius: i32) -> JavaObjectHashSet<BlockPos> {
        let surface = self.place_ground_patch(level, random, origin, x_radius, z_radius);
        let mut water_surface = JavaObjectHashSet::default();
        for surface_pos in surface.iter() {
            if !is_exposed(level, surface_pos) {
                water_surface.insert(surface_pos);
            }
        }
        for surface_pos in water_surface.iter() {
            level.set_block(surface_pos, blocks::WATER.default_state(), 2);
        }
        water_surface
    }

    fn distribute_vegetation<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, surface: &JavaObjectHashSet<BlockPos>) {
        for surface_pos in surface.iter() {
            if self.vegetation_chance > 0.0 && random.next_float() < self.vegetation_chance {
                self.place_vegetation(level, random, surface_pos);
            }
        }
    }

    fn place_vegetation<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, vegetation_pos: BlockPos) -> bool {
        if self.waterlogged {
            if self.vegetation_patch_place_vegetation(level, random, vegetation_pos.below()) {
                let placed = level.get_block_state(vegetation_pos);
                if placed.has(WATERLOGGED) && !placed.get(WATERLOGGED) {
                    level.set_block(vegetation_pos, placed.set(WATERLOGGED, true), 2);
                }
                true
            } else {
                false
            }
        } else {
            self.vegetation_patch_place_vegetation(level, random, vegetation_pos)
        }
    }

    fn vegetation_patch_place_vegetation<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, vegetation_pos: BlockPos) -> bool {
        self.vegetation_feature.value().place(level, random, vegetation_pos.relative(self.surface.opposite()))
    }

    fn place_ground<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, mut below_pos: BlockPos, depth: i32) -> bool {
        for i in 0..depth {
            let state_to_place: BlockState = self.ground_state.value().get_state(level, random, below_pos);
            let below_state = level.get_block_state(below_pos);
            if !state_to_place.is_block(below_state.block()) {
                if !self.replaceable.contains(below_state.block()) {
                    return i != 0;
                }
                level.set_block(below_pos, state_to_place, 2);
                below_pos = below_pos.relative(self.surface);
            }
        }
        true
    }
}

fn is_exposed(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    [Direction::North, Direction::East, Direction::South, Direction::West, Direction::Down].into_iter().any(|direction| is_exposed_direction(level, pos, direction))
}

fn is_exposed_direction(level: &mut WorldGenRegion, pos: BlockPos, direction: Direction) -> bool {
    !level.get_block_state(pos.relative(direction)).is_face_sturdy_full(direction.opposite())
}
