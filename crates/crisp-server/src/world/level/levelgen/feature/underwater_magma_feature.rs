use crisp_core::random::RandomSource;
use crisp_core::shapes;
use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::column::Column;

pub struct UnderwaterMagmaFeature {
    floor_search_range: i32,
    placement_radius_around_floor: i32,
    placement_probability_per_valid_position: f32,
}

impl UnderwaterMagmaFeature {
    pub fn parse(json: &Value) -> Result<UnderwaterMagmaFeature, String> {
        let int = |key: &str| json.get(key).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| format!("missing {key}"));
        Ok(UnderwaterMagmaFeature {
            floor_search_range: int("floor_search_range")?,
            placement_radius_around_floor: int("placement_radius_around_floor")?,
            placement_probability_per_valid_position: float(json.get("placement_probability_per_valid_position").ok_or("missing placement_probability_per_valid_position")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let Some(floor_y) = self.get_floor_y(level, origin) else {
            return false;
        };
        let floor_pos = BlockPos::new(origin.x, floor_y, origin.z);
        let r = self.placement_radius_around_floor;
        let mut placed = 0;
        for z in floor_pos.z - r..=floor_pos.z + r {
            for y in floor_pos.y - r..=floor_pos.y + r {
                for x in floor_pos.x - r..=floor_pos.x + r {
                    let pos = BlockPos::new(x, y, z);
                    if random.next_float() < self.placement_probability_per_valid_position && self.is_valid_placement(level, pos) {
                        level.set_block(pos, blocks::MAGMA_BLOCK.default_state(), 2);
                        placed += 1;
                    }
                }
            }
        }
        placed > 0
    }

    fn get_floor_y(&self, level: &mut WorldGenRegion, origin: BlockPos) -> Option<i32> {
        Column::scan(level, origin, self.floor_search_range, |state| state.is_block(blocks::WATER), |state| !state.is_block(blocks::WATER))?.get_floor()
    }

    fn is_valid_placement(&self, level: &mut WorldGenRegion, pos: BlockPos) -> bool {
        if is_water_or_air(level.get_block_state(pos)) || is_visible_from_outside(level, pos.below(), Direction::Up) {
            return false;
        }
        for neighbour_dir in Direction::PLANE_HORIZONTAL {
            if is_visible_from_outside(level, pos.relative(neighbour_dir), neighbour_dir.opposite()) {
                return false;
            }
        }
        true
    }
}

fn is_water_or_air(state: BlockState) -> bool {
    state.is_block(blocks::WATER) || state.is_air()
}

fn is_visible_from_outside(level: &mut WorldGenRegion, pos: BlockPos, covered_direction: Direction) -> bool {
    let state = level.get_block_state(pos);
    !(state.can_occlude() && shapes::is_face_full(state.occlusion_voxel_shape(), covered_direction))
}
