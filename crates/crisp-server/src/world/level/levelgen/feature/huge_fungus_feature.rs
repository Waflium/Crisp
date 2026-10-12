use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use crisp_data::properties::AGE_25;
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;
use crate::world::level::levelgen::parse_block_state;
use crate::world::level::world_gen_level::WorldGenLevel;

const HUGE_PROBABILITY: f32 = 0.06;

pub struct HugeFungusFeature {
    valid_base_state: BlockState,
    stem_state: BlockState,
    hat_state: BlockState,
    decor_state: BlockState,
    replaceable_blocks: BlockPredicate,
    planted: bool,
}

impl HugeFungusFeature {
    pub fn parse(json: &Value) -> Result<HugeFungusFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(HugeFungusFeature {
            valid_base_state: parse_block_state(field("valid_base_block")?)?,
            stem_state: parse_block_state(field("stem_state")?)?,
            hat_state: parse_block_state(field("hat_state")?)?,
            decor_state: parse_block_state(field("decor_state")?)?,
            replaceable_blocks: BlockPredicate::parse(field("replaceable_blocks")?)?,
            planted: json.get("planted").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let allowed_base_block = self.valid_base_state.block();
        if !level.get_block_state(origin.below()).is_block(allowed_base_block) {
            return false;
        }
        let new_origin = origin;
        let mut total_height = mth::next_int(random, 4, 13);
        if random.next_int_bounded(12) == 0 {
            total_height *= 2;
        }
        if !self.planted {
            let max_height = level.generator().gen_depth();
            if new_origin.y + total_height + 1 >= max_height {
                return false;
            }
        }
        let is_huge = !self.planted && random.next_float() < HUGE_PROBABILITY;
        level.set_block(origin, blocks::AIR.default_state(), 260);
        self.place_stem(level, random, new_origin, total_height, is_huge);
        self.place_hat(level, random, new_origin, total_height, is_huge);
        true
    }

    fn is_replaceable(&self, level: &mut impl WorldGenLevel, pos: BlockPos, check_non_replaceable_plants: bool) -> bool {
        level.get_block_state(pos).can_be_replaced() || check_non_replaceable_plants && self.replaceable_blocks.test(level, pos)
    }

    fn place_stem<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, surface_origin: BlockPos, total_height: i32, is_huge: bool) {
        let stem_radius: i32 = if is_huge { 1 } else { 0 };
        for dx in -stem_radius..=stem_radius {
            for dz in -stem_radius..=stem_radius {
                let corner_of_huge_stem = is_huge && dx.abs() == stem_radius && dz.abs() == stem_radius;
                for dy in 0..total_height {
                    let block_pos = surface_origin.offset(dx, dy, dz);
                    if self.is_replaceable(level, block_pos, true) {
                        if self.planted {
                            if !level.get_block_state(block_pos.below()).is_air() {
                                level.destroy_block(block_pos, true);
                            }
                            level.set_block_and_update(block_pos, self.stem_state);
                        } else if corner_of_huge_stem {
                            if random.next_float() < 0.1 {
                                level.set_block_and_update(block_pos, self.stem_state);
                            }
                        } else {
                            level.set_block_and_update(block_pos, self.stem_state);
                        }
                    }
                }
            }
        }
    }

    fn place_hat<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, surface_origin: BlockPos, total_height: i32, is_huge: bool) {
        let place_vines = self.hat_state.is_block(blocks::NETHER_WART_BLOCK);
        let hat_height = (random.next_int_bounded(1 + total_height / 3) + 5).min(total_height);
        let hat_start_y = total_height - hat_height;
        for dy in hat_start_y..=total_height {
            let mut radius = if dy < total_height - random.next_int_bounded(3) { 2 } else { 1 };
            if hat_height > 8 && dy < hat_start_y + 4 {
                radius = 3;
            }
            if is_huge {
                radius += 1;
            }
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let is_edge_x = dx == -radius || dx == radius;
                    let is_edge_z = dz == -radius || dz == radius;
                    let inside = !is_edge_x && !is_edge_z && dy != total_height;
                    let corner = is_edge_x && is_edge_z;
                    let is_hat_bottom = dy < hat_start_y + 3;
                    let block_pos = surface_origin.offset(dx, dy, dz);
                    if self.is_replaceable(level, block_pos, false) {
                        if self.planted && !level.get_block_state(block_pos.below()).is_air() {
                            level.destroy_block(block_pos, true);
                        }
                        if is_hat_bottom {
                            if !inside {
                                self.place_hat_drop_block(level, random, block_pos, place_vines);
                            }
                        } else if inside {
                            self.place_hat_block(level, random, block_pos, 0.1, 0.2, if place_vines { 0.1 } else { 0.0 });
                        } else if corner {
                            self.place_hat_block(level, random, block_pos, 0.01, 0.7, if place_vines { 0.083 } else { 0.0 });
                        } else {
                            self.place_hat_block(level, random, block_pos, 5.0e-4, 0.98, if place_vines { 0.07 } else { 0.0 });
                        }
                    }
                }
            }
        }
    }

    fn place_hat_block<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        random: &mut R,
        block_pos: BlockPos,
        decor_block_probability: f32,
        hat_block_probability: f32,
        vines_probability: f32,
    ) {
        if random.next_float() < decor_block_probability {
            level.set_block_and_update(block_pos, self.decor_state);
        } else if random.next_float() < hat_block_probability {
            level.set_block_and_update(block_pos, self.hat_state);
            if random.next_float() < vines_probability {
                try_place_weeping_vines(block_pos, level, random);
            }
        }
    }

    fn place_hat_drop_block<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, block_pos: BlockPos, place_vines: bool) {
        if level.get_block_state(block_pos.below()).is_block(self.hat_state.block()) {
            level.set_block_and_update(block_pos, self.hat_state);
        } else if (random.next_float() as f64) < 0.15 {
            level.set_block_and_update(block_pos, self.hat_state);
            if place_vines && random.next_int_bounded(11) == 0 {
                try_place_weeping_vines(block_pos, level, random);
            }
        }
    }
}

fn try_place_weeping_vines<R: RandomSource>(hat_block_pos: BlockPos, level: &mut impl WorldGenLevel, random: &mut R) {
    let place_pos = hat_block_pos.below();
    if level.get_block_state(place_pos).is_air() {
        let mut goal_vine_height = mth::next_int(random, 1, 5);
        if random.next_int_bounded(7) == 0 {
            goal_vine_height *= 2;
        }
        place_weeping_vines_column(level, random, place_pos, goal_vine_height, 23, 25);
    }
}

fn place_weeping_vines_column<R: RandomSource>(level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos, total_height: i32, min_age: i32, max_age: i32) {
    let mut place_pos = origin;
    for height in 0..=total_height {
        if level.get_block_state(place_pos).is_air() {
            if height == total_height || !level.get_block_state(place_pos.below()).is_air() {
                let age = mth::next_int(random, min_age, max_age);
                level.set_block(place_pos, blocks::WEEPING_VINES.default_state().set(AGE_25, age), 2);
                break;
            }
            level.set_block(place_pos, blocks::WEEPING_VINES_PLANT.default_state(), 2);
        }
        place_pos = place_pos.below();
    }
}
