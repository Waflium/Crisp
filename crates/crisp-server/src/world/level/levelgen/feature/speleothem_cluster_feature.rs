use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, mth};
use crisp_data::{BlockState, block_tags, blocks, fluid_tags};
use serde_json::Value;

use super::speleothem_utils;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::{FloatProvider, IntProvider, float, normal};
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;
use crate::world::level::levelgen::column::Column;
use crate::world::level::levelgen::parse_block_state;

pub struct SpeleothemClusterFeature {
    base_block: BlockState,
    pointed_block: BlockState,
    replaceable_blocks: BlockSet,
    floor_to_ceiling_search_range: i32,
    height: IntProvider,
    radius: IntProvider,
    max_stalagmite_stalactite_height_diff: i32,
    height_deviation: i32,
    speleothem_block_layer_thickness: IntProvider,
    density: FloatProvider,
    wetness: FloatProvider,
    chance_of_speleothem_at_max_distance_from_center: f32,
    max_distance_from_edge_affecting_chance_of_speleothem: i32,
    max_distance_from_center_affecting_height_bias: i32,
}

impl SpeleothemClusterFeature {
    pub fn parse(json: &Value) -> Result<SpeleothemClusterFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let int = |key: &str| field(key)?.as_i64().map(|v| v as i32).ok_or_else(|| format!("bad {key}"));
        Ok(SpeleothemClusterFeature {
            base_block: parse_block_state(field("base_block")?)?,
            pointed_block: parse_block_state(field("pointed_block")?)?,
            replaceable_blocks: BlockSet::parse(field("replaceable_blocks")?)?,
            floor_to_ceiling_search_range: int("floor_to_ceiling_search_range")?,
            height: IntProvider::parse(field("height")?)?,
            radius: IntProvider::parse(field("radius")?)?,
            max_stalagmite_stalactite_height_diff: int("max_stalagmite_stalactite_height_diff")?,
            height_deviation: int("height_deviation")?,
            speleothem_block_layer_thickness: IntProvider::parse(field("speleothem_block_layer_thickness")?)?,
            density: FloatProvider::parse(field("density")?)?,
            wetness: FloatProvider::parse(field("wetness")?)?,
            chance_of_speleothem_at_max_distance_from_center: float(field("chance_of_speleothem_at_max_distance_from_center")?)?,
            max_distance_from_edge_affecting_chance_of_speleothem: int("max_distance_from_edge_affecting_chance_of_speleothem")?,
            max_distance_from_center_affecting_height_bias: int("max_distance_from_center_affecting_height_bias")?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        if !speleothem_utils::is_empty_or_water_at(level, origin) {
            return false;
        }
        let height = self.height.sample(random);
        let wetness = self.wetness.sample(random);
        let density = self.density.sample(random);
        let x_radius = self.radius.sample(random);
        let z_radius = self.radius.sample(random);
        for dx in -x_radius..=x_radius {
            for dz in -z_radius..=z_radius {
                let chance_of_stalagmite_or_stalactite = self.get_chance_of_stalagmite_or_stalactite(x_radius, z_radius, dx, dz);
                let pos = origin.offset(dx, 0, dz);
                self.place_column(level, random, pos, dx, dz, wetness, chance_of_stalagmite_or_stalactite, height, density);
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn place_column<R: RandomSource>(
        &self,
        level: &mut WorldGenRegion,
        random: &mut R,
        pos: BlockPos,
        dx: i32,
        dz: i32,
        chance_of_water: f32,
        chance_of_stalagmite_or_stalactite: f64,
        cluster_height: i32,
        density: f32,
    ) {
        let Some(base_column) = Column::scan(level, pos, self.floor_to_ceiling_search_range, speleothem_utils::is_empty_or_water, speleothem_utils::is_neither_empty_nor_water) else {
            return;
        };
        let ceiling = base_column.get_ceiling();
        let base_floor = base_column.get_floor();
        if ceiling.is_none() && base_floor.is_none() {
            return;
        }
        let at_y = |y: i32| BlockPos::new(pos.x, y, pos.z);
        let want_pool = random.next_float() < chance_of_water;
        let column = match base_floor {
            Some(base_floor_y) if want_pool && self.can_place_pool(level, at_y(base_floor_y)) => {
                let column = base_column.with_floor(Some(base_floor_y - 1));
                level.set_block(at_y(base_floor_y), blocks::WATER.default_state(), 2);
                column
            }
            _ => base_column,
        };
        let floor = column.get_floor();
        let want_stalactite = random.next_double() < chance_of_stalagmite_or_stalactite;
        let stalactite_height = match ceiling {
            Some(ceiling_y) if want_stalactite && !self.is_lava(level, at_y(ceiling_y)) => {
                let ceiling_thickness = self.speleothem_block_layer_thickness.sample(random);
                self.replace_blocks_with_base_blocks(level, at_y(ceiling_y), ceiling_thickness, Direction::Up);
                let max_height_for_this_column = match floor {
                    Some(floor_y) => cluster_height.min(ceiling_y - floor_y),
                    None => cluster_height,
                };
                self.get_speleothem_height(random, dx, dz, density, max_height_for_this_column)
            }
            _ => 0,
        };
        let want_stalagmite = random.next_double() < chance_of_stalagmite_or_stalactite;
        let stalagmite_height = match floor {
            Some(floor_y) if want_stalagmite && !self.is_lava(level, at_y(floor_y)) => {
                let floor_thickness = self.speleothem_block_layer_thickness.sample(random);
                self.replace_blocks_with_base_blocks(level, at_y(floor_y), floor_thickness, Direction::Down);
                if ceiling.is_some() {
                    0.max(stalactite_height + mth::random_between_inclusive(random, -self.max_stalagmite_stalactite_height_diff, self.max_stalagmite_stalactite_height_diff))
                } else {
                    self.get_speleothem_height(random, dx, dz, density, cluster_height)
                }
            }
            _ => 0,
        };
        let (actual_stalactite_height, actual_stalagmite_height) = match (ceiling, floor) {
            (Some(ceiling_y), Some(floor_y)) if ceiling_y - stalactite_height <= floor_y + stalagmite_height => {
                let lowest_stalactite_bottom = (ceiling_y - stalactite_height).max(floor_y + 1);
                let highest_stalagmite_top = (floor_y + stalagmite_height).min(ceiling_y - 1);
                let actual_stalactite_bottom = mth::random_between_inclusive(random, lowest_stalactite_bottom, highest_stalagmite_top + 1);
                let actual_stalagmite_top = actual_stalactite_bottom - 1;
                (ceiling_y - actual_stalactite_bottom, actual_stalagmite_top - floor_y)
            }
            _ => (stalactite_height, stalagmite_height),
        };
        let merge_tips = random.next_boolean()
            && actual_stalactite_height > 0
            && actual_stalagmite_height > 0
            && column.get_height().is_some_and(|height| actual_stalactite_height + actual_stalagmite_height == height);
        let base = self.base_block.block();
        let pointed = self.pointed_block.block();
        if let Some(ceiling_y) = ceiling {
            speleothem_utils::grow_speleothem(level, at_y(ceiling_y - 1), Direction::Down, actual_stalactite_height, merge_tips, base, pointed, &self.replaceable_blocks);
        }
        if let Some(floor_y) = floor {
            speleothem_utils::grow_speleothem(level, at_y(floor_y + 1), Direction::Up, actual_stalagmite_height, merge_tips, base, pointed, &self.replaceable_blocks);
        }
    }

    fn is_lava(&self, level: &mut WorldGenRegion, pos: BlockPos) -> bool {
        level.get_block_state(pos).is_block(blocks::LAVA)
    }

    fn get_speleothem_height(&self, random: &mut impl RandomSource, dx: i32, dz: i32, density: f32, max_height: i32) -> i32 {
        if random.next_float() > density {
            return 0;
        }
        let distance_from_center = dx.abs() + dz.abs();
        let height_mean = mth::clamped_map(distance_from_center as f64, 0.0, self.max_distance_from_center_affecting_height_bias as f64, max_height as f64 / 2.0, 0.0) as f32;
        random_between_biased(random, 0.0, max_height as f32, height_mean, self.height_deviation as f32) as i32
    }

    fn can_place_pool(&self, level: &mut WorldGenRegion, pos: BlockPos) -> bool {
        let state = level.get_block_state(pos);
        if state.is_block(blocks::WATER) || state.is_block(self.base_block.block()) || state.is_block(self.pointed_block.block()) {
            return false;
        }
        if level.get_block_state(pos.above()).fluid_state().is(&fluid_tags::WATER) {
            return false;
        }
        for direction in Direction::PLANE_HORIZONTAL {
            if !can_be_adjacent_to_water(level, pos.relative(direction)) {
                return false;
            }
        }
        can_be_adjacent_to_water(level, pos.below())
    }

    fn replace_blocks_with_base_blocks(&self, level: &mut WorldGenRegion, first_pos: BlockPos, max_count: i32, direction: Direction) {
        let mut pos = first_pos;
        for _ in 0..max_count {
            if !speleothem_utils::place_base_block_if_possible(level, pos, self.base_block.block(), &self.replaceable_blocks) {
                return;
            }
            pos = pos.relative(direction);
        }
    }

    fn get_chance_of_stalagmite_or_stalactite(&self, x_radius: i32, z_radius: i32, dx: i32, dz: i32) -> f64 {
        let x_distance_from_edge = x_radius - dx.abs();
        let z_distance_from_edge = z_radius - dz.abs();
        let distance_from_edge = x_distance_from_edge.min(z_distance_from_edge);
        mth::clamped_map_f(distance_from_edge as f32, 0.0, self.max_distance_from_edge_affecting_chance_of_speleothem as f32, self.chance_of_speleothem_at_max_distance_from_center, 1.0) as f64
    }
}

fn can_be_adjacent_to_water(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    let state = level.get_block_state(pos);
    state.is(&block_tags::BASE_STONE_OVERWORLD) || state.fluid_state().is(&fluid_tags::WATER)
}

fn random_between_biased(random: &mut impl RandomSource, min: f32, max_exclusive: f32, mean: f32, deviation: f32) -> f32 {
    let value = normal(random, mean, deviation);
    if value < min { min } else { value.min(max_exclusive) }
}
