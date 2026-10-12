use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::{block_tags, blocks};
use serde_json::Value;

use super::speleothem_utils;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::{FloatProvider, IntProvider, float, random_between};
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;
use crate::world::level::levelgen::column::Column;

pub struct LargeDripstoneFeature {
    replaceable_blocks: BlockSet,
    floor_to_ceiling_search_range: i32,
    column_radius: IntProvider,
    height_scale: FloatProvider,
    max_column_radius_to_cave_height_ratio: f32,
    stalactite_bluntness: FloatProvider,
    stalagmite_bluntness: FloatProvider,
    wind_speed: FloatProvider,
    min_radius_for_wind: i32,
    min_bluntness_for_wind: f32,
}

impl LargeDripstoneFeature {
    pub fn parse(json: &Value) -> Result<LargeDripstoneFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(LargeDripstoneFeature {
            replaceable_blocks: BlockSet::parse(field("replaceable_blocks")?)?,
            floor_to_ceiling_search_range: json.get("floor_to_ceiling_search_range").and_then(Value::as_i64).map_or(30, |v| v as i32),
            column_radius: IntProvider::parse(field("column_radius")?)?,
            height_scale: FloatProvider::parse(field("height_scale")?)?,
            max_column_radius_to_cave_height_ratio: float(field("max_column_radius_to_cave_height_ratio")?)?,
            stalactite_bluntness: FloatProvider::parse(field("stalactite_bluntness")?)?,
            stalagmite_bluntness: FloatProvider::parse(field("stalagmite_bluntness")?)?,
            wind_speed: FloatProvider::parse(field("wind_speed")?)?,
            min_radius_for_wind: field("min_radius_for_wind")?.as_i64().ok_or("bad min_radius_for_wind")? as i32,
            min_bluntness_for_wind: float(field("min_bluntness_for_wind")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        if !speleothem_utils::is_empty_or_water_at(level, origin) {
            return false;
        }
        let column = Column::scan(level, origin, self.floor_to_ceiling_search_range, speleothem_utils::is_empty_or_water, |state| {
            speleothem_utils::is_base_or_lava(state, blocks::DRIPSTONE_BLOCK, &self.replaceable_blocks)
        });
        let Some(Column::Range { floor, ceiling }) = column else {
            return false;
        };
        let height = ceiling - floor - 1;
        if height < 4 {
            return false;
        }
        let max_column_radius_based_on_column_height = (height as f32 * self.max_column_radius_to_cave_height_ratio) as i32;
        let max_column_radius = max_column_radius_based_on_column_height.clamp(self.column_radius.min_inclusive(), self.column_radius.max_inclusive());
        let radius = mth::random_between_inclusive(random, self.column_radius.min_inclusive(), max_column_radius);
        let at_y = |y: i32| BlockPos::new(origin.x, y, origin.z);
        let mut stalactite = make_dripstone(at_y(ceiling - 1), false, random, radius, &self.stalactite_bluntness, &self.height_scale);
        let mut stalagmite = make_dripstone(at_y(floor + 1), true, random, radius, &self.stalagmite_bluntness, &self.height_scale);
        let wind = if stalactite.is_suitable_for_wind(self.min_radius_for_wind, self.min_bluntness_for_wind) && stalagmite.is_suitable_for_wind(self.min_radius_for_wind, self.min_bluntness_for_wind) {
            WindOffsetter::new(origin.y, random, &self.wind_speed, 16 - radius)
        } else {
            WindOffsetter::no_wind()
        };
        let stalactite_base_embedded_in_stone = stalactite.move_back_until_base_is_inside_stone_and_shrink_radius_if_necessary(level, &wind);
        let stalagmite_base_embedded_in_stone = stalagmite.move_back_until_base_is_inside_stone_and_shrink_radius_if_necessary(level, &wind);
        if stalactite_base_embedded_in_stone {
            stalactite.place_blocks(level, random, &wind);
        }
        if stalagmite_base_embedded_in_stone {
            stalagmite.place_blocks(level, random, &wind);
        }
        true
    }
}

fn make_dripstone(root: BlockPos, pointing_up: bool, random: &mut impl RandomSource, radius: i32, bluntness: &FloatProvider, height_scale: &FloatProvider) -> LargeDripstone {
    let bluntness = bluntness.sample(random) as f64;
    let scale = height_scale.sample(random) as f64;
    LargeDripstone { root, pointing_up, radius, bluntness, scale }
}

struct LargeDripstone {
    root: BlockPos,
    pointing_up: bool,
    radius: i32,
    bluntness: f64,
    scale: f64,
}

impl LargeDripstone {
    fn get_height(&self) -> i32 {
        self.get_height_at_radius(0.0)
    }

    fn move_back_until_base_is_inside_stone_and_shrink_radius_if_necessary(&mut self, level: &mut impl WorldGenLevel, wind: &WindOffsetter) -> bool {
        while self.radius > 1 {
            let mut new_root = self.root;
            let max_tries = 10.min(self.get_height());
            for _ in 0..max_tries {
                if level.get_block_state(new_root).is_block(blocks::LAVA) {
                    return false;
                }
                if speleothem_utils::is_circle_mostly_embedded_in_stone(level, wind.offset(new_root), self.radius) {
                    self.root = new_root;
                    return true;
                }
                new_root = if self.pointing_up { new_root.below() } else { new_root.above() };
            }
            self.radius /= 2;
        }
        false
    }

    fn get_height_at_radius(&self, check_radius: f32) -> i32 {
        speleothem_utils::get_speleothem_height(check_radius as f64, self.radius as f64, self.scale, self.bluntness) as i32
    }

    fn place_blocks(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, wind: &WindOffsetter) {
        for dx in -self.radius..=self.radius {
            for dz in -self.radius..=self.radius {
                let current_radius = mth::sqrt((dx * dx + dz * dz) as f32);
                if current_radius <= self.radius as f32 {
                    let mut height = self.get_height_at_radius(current_radius);
                    if height > 0 {
                        if (random.next_float() as f64) < 0.2 {
                            height = (height as f32 * random_between(random, 0.8, 1.0)) as i32;
                        }
                        let mut pos = self.root.offset(dx, 0, dz);
                        let mut has_been_out_of_stone = false;
                        let max_y = if self.pointing_up { level.get_height(HeightmapType::WorldSurfaceWg, pos.x, pos.z) } else { i32::MAX };
                        let mut i = 0;
                        while i < height && pos.y < max_y {
                            let wind_adjusted_pos = wind.offset(pos);
                            if speleothem_utils::is_empty_or_water_or_lava_at(level, wind_adjusted_pos) {
                                has_been_out_of_stone = true;
                                level.set_block(wind_adjusted_pos, blocks::DRIPSTONE_BLOCK.default_state(), 2);
                            } else if has_been_out_of_stone && level.get_block_state(wind_adjusted_pos).is(&block_tags::BASE_STONE_OVERWORLD) {
                                break;
                            }
                            pos = if self.pointing_up { pos.above() } else { pos.below() };
                            i += 1;
                        }
                    }
                }
            }
        }
    }

    fn is_suitable_for_wind(&self, min_radius_for_wind: i32, min_bluntness_for_wind: f32) -> bool {
        self.radius >= min_radius_for_wind && self.bluntness >= min_bluntness_for_wind as f64
    }
}

struct WindOffsetter {
    origin_y: i32,
    wind_speed: Option<Vec3>,
    max_offset: i32,
}

impl WindOffsetter {
    fn new(origin_y: i32, random: &mut impl RandomSource, wind_speed_range: &FloatProvider, max_offset: i32) -> WindOffsetter {
        let speed = wind_speed_range.sample(random);
        let direction = random_between(random, 0.0, std::f64::consts::PI as f32);
        let wind_speed = Vec3::new((mth::cos(direction as f64) * speed) as f64, 0.0, (mth::sin(direction as f64) * speed) as f64);
        WindOffsetter { origin_y, wind_speed: Some(wind_speed), max_offset }
    }

    fn no_wind() -> WindOffsetter {
        WindOffsetter { origin_y: 0, wind_speed: None, max_offset: 0 }
    }

    fn offset(&self, pos: BlockPos) -> BlockPos {
        let Some(wind_speed) = self.wind_speed else {
            return pos;
        };
        let dy = (self.origin_y - pos.y) as f64;
        let dx = mth::floor(wind_speed.x * dy).clamp(-self.max_offset, self.max_offset);
        let dz = mth::floor(wind_speed.z * dy).clamp(-self.max_offset, self.max_offset);
        pos.offset(dx, 0, dz)
    }
}
