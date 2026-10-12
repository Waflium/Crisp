use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{blocks, fluid_tags};
use serde_json::Value;

use super::feature::mark_above_for_post_processing;
use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;

pub struct LakeFeature {
    fluid: BlockStateProviderHolder,
    barrier: BlockStateProviderHolder,
    can_place_feature: BlockPredicate,
    can_replace_with_air_or_fluid: BlockPredicate,
    can_replace_with_barrier: BlockPredicate,
}

fn index(xx: i32, zz: i32, yy: i32) -> usize {
    ((xx * 16 + zz) * 8 + yy) as usize
}

fn is_edge(grid: &[bool; 2048], xx: i32, zz: i32, yy: i32) -> bool {
    !grid[index(xx, zz, yy)]
        && (xx < 15 && grid[index(xx + 1, zz, yy)]
            || xx > 0 && grid[index(xx - 1, zz, yy)]
            || zz < 15 && grid[index(xx, zz + 1, yy)]
            || zz > 0 && grid[index(xx, zz - 1, yy)]
            || yy < 7 && grid[index(xx, zz, yy + 1)]
            || yy > 0 && grid[index(xx, zz, yy - 1)])
}

impl LakeFeature {
    pub fn parse(json: &Value) -> Result<LakeFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(LakeFeature {
            fluid: BlockStateProviderHolder::parse(field("fluid")?)?,
            barrier: BlockStateProviderHolder::parse(field("barrier")?)?,
            can_place_feature: BlockPredicate::parse(field("can_place_feature")?)?,
            can_replace_with_air_or_fluid: BlockPredicate::parse(field("can_replace_with_air_or_fluid")?)?,
            can_replace_with_barrier: BlockPredicate::parse(field("can_replace_with_barrier")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        if origin.y <= level.get_min_y() + 4 {
            return false;
        }
        let origin = origin.offset(-8, -4, -8);
        let mut grid = [false; 2048];
        let spots = random.next_int_bounded(4) + 4;
        for _ in 0..spots {
            let xr = random.next_double() * 6.0 + 3.0;
            let yr = random.next_double() * 4.0 + 2.0;
            let zr = random.next_double() * 6.0 + 3.0;
            let xp = random.next_double() * (16.0 - xr - 2.0) + 1.0 + xr / 2.0;
            let yp = random.next_double() * (8.0 - yr - 4.0) + 2.0 + yr / 2.0;
            let zp = random.next_double() * (16.0 - zr - 2.0) + 1.0 + zr / 2.0;
            for xx in 1..15 {
                for zz in 1..15 {
                    for yy in 1..7 {
                        let xd = (xx as f64 - xp) / (xr / 2.0);
                        let yd = (yy as f64 - yp) / (yr / 2.0);
                        let zd = (zz as f64 - zp) / (zr / 2.0);
                        let d = xd * xd + yd * yd + zd * zd;
                        if d < 1.0 {
                            grid[index(xx, zz, yy)] = true;
                        }
                    }
                }
            }
        }
        let fluid = self.fluid.value().get_state(level, random, origin);
        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if is_edge(&grid, xx, zz, yy) {
                        let offset_pos = origin.offset(xx, yy, zz);
                        let block_state = level.get_block_state(offset_pos);
                        if yy >= 4 && block_state.liquid() {
                            return false;
                        }
                        if yy < 4 && !block_state.is_solid() && block_state != fluid {
                            return false;
                        }
                        if !self.can_place_feature.test(level, offset_pos) {
                            return false;
                        }
                    }
                }
            }
        }
        let air = blocks::CAVE_AIR.default_state();
        for xx in 0..16 {
            for zz in 0..16 {
                for yy in 0..8 {
                    if grid[index(xx, zz, yy)] {
                        let place_pos = origin.offset(xx, yy, zz);
                        if self.can_replace_with_air_or_fluid.test(level, place_pos) {
                            let place_air = yy >= 4;
                            level.set_block(place_pos, if place_air { air } else { fluid }, 2);
                            if place_air {
                                level.schedule_tick(place_pos, air.block(), 0);
                                mark_above_for_post_processing(level, place_pos);
                            }
                        }
                    }
                }
            }
        }
        let barrier = self.barrier.value().get_state(level, random, origin);
        if !barrier.is_air() {
            for xx in 0..16 {
                for zz in 0..16 {
                    for yy in 0..8 {
                        if is_edge(&grid, xx, zz, yy) && (yy < 4 || random.next_int_bounded(2) != 0) {
                            let offset = origin.offset(xx, yy, zz);
                            let block_state = level.get_block_state(offset);
                            if block_state.is_solid() && self.can_replace_with_barrier.test(level, offset) {
                                level.set_block(offset, barrier, 2);
                                mark_above_for_post_processing(level, offset);
                            }
                        }
                    }
                }
            }
        }
        if fluid.fluid_state().is(&fluid_tags::WATER) {
            for xx in 0..16 {
                for zz in 0..16 {
                    let offset = origin.offset(xx, 4, zz);
                    let biome = level.get_biome(offset);
                    if biome.should_freeze(level, offset, false) && self.can_replace_with_air_or_fluid.test(level, offset) {
                        level.set_block(offset, blocks::ICE.default_state(), 2);
                    }
                }
            }
        }
        true
    }
}
