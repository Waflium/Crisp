use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockPredicate, BlockSet};
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct SteppedColumnClusterFeature {
    block: BlockStateProviderHolder,
    continue_through: BlockPredicate,
    can_replace: BlockPredicate,
    cannot_place_on: BlockSet,
    cluster_reach: IntProvider,
    column_count: IntProvider,
    column_reach: IntProvider,
    height: IntProvider,
}

impl SteppedColumnClusterFeature {
    pub fn parse(json: &Value) -> Result<SteppedColumnClusterFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(SteppedColumnClusterFeature {
            block: BlockStateProviderHolder::parse(field("block")?)?,
            continue_through: BlockPredicate::parse(field("continue_through")?)?,
            can_replace: BlockPredicate::parse(field("can_replace")?)?,
            cannot_place_on: BlockSet::parse(field("cannot_place_on")?)?,
            cluster_reach: IntProvider::parse(field("cluster_reach")?)?,
            column_count: IntProvider::parse(field("column_count")?)?,
            column_reach: IntProvider::parse(field("column_reach")?)?,
            height: IntProvider::parse(field("height")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let lava_sea_level = level.generator().sea_level();
        if !self.can_place_at(level, origin) {
            return false;
        }
        let column_height = self.height.sample(random);
        let cluster_reach = column_height.min(self.cluster_reach.sample(random));
        let count = self.column_count.sample(random);
        let mut placed = false;
        let (min_x, min_z) = (origin.x - cluster_reach, origin.z - cluster_reach);
        let (width, depth) = (2 * cluster_reach + 1, 2 * cluster_reach + 1);
        for _ in 0..count {
            let x = min_x + random.next_int_bounded(width);
            let y = origin.y + random.next_int_bounded(1);
            let z = min_z + random.next_int_bounded(depth);
            let pos = BlockPos::new(x, y, z);
            let blocks_to_place_y = column_height - pos.dist_manhattan(origin);
            if blocks_to_place_y >= 0 {
                let reach = self.column_reach.sample(random);
                placed |= self.place_column(level, random, lava_sea_level, pos, blocks_to_place_y, reach);
            }
        }
        placed
    }

    fn place_column<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, lava_sea_level: i32, origin: BlockPos, column_height: i32, reach: i32) -> bool {
        let mut placed_any = false;
        for pos in BlockPos::between_closed(BlockPos::new(origin.x - reach, origin.y, origin.z - reach), BlockPos::new(origin.x + reach, origin.y, origin.z + reach)) {
            let step_limit = pos.dist_manhattan(origin);
            let column_pos = if self.can_replace.test(level, pos) {
                self.find_surface(level, lava_sea_level, pos, step_limit)
            } else {
                self.find_air(level, pos, step_limit)
            };
            if let Some(column_pos) = column_pos {
                let mut blocks_y = column_height - step_limit / 2;
                let mut cursor = column_pos;
                while blocks_y >= 0 {
                    if self.can_replace.test(level, cursor) {
                        let state = self.block.value().get_state(level, random, cursor);
                        level.set_block_and_update(cursor, state);
                        cursor = cursor.above();
                        placed_any = true;
                    } else {
                        if !self.continue_through.test(level, cursor) {
                            break;
                        }
                        cursor = cursor.above();
                    }
                    blocks_y -= 1;
                }
            }
        }
        placed_any
    }

    fn find_surface(&self, level: &mut impl WorldGenLevel, _lava_sea_level: i32, mut cursor: BlockPos, mut limit: i32) -> Option<BlockPos> {
        while cursor.y > level.get_min_y() + 1 && limit > 0 {
            limit -= 1;
            if self.can_place_at(level, cursor) {
                return Some(cursor);
            }
            cursor = cursor.below();
        }
        None
    }

    fn can_place_at(&self, level: &mut impl WorldGenLevel, cursor: BlockPos) -> bool {
        if !self.can_replace.test(level, cursor) {
            return false;
        }
        let block_state = level.get_block_state(cursor.below());
        !block_state.is_air() && !self.cannot_place_on.contains(block_state.block())
    }

    fn find_air(&self, level: &mut impl WorldGenLevel, mut cursor: BlockPos, mut limit: i32) -> Option<BlockPos> {
        while cursor.y <= level.get_max_y() && limit > 0 {
            limit -= 1;
            let block_state = level.get_block_state(cursor);
            if self.cannot_place_on.contains(block_state.block()) {
                return None;
            }
            if block_state.is_air() {
                return Some(cursor);
            }
            cursor = cursor.above();
        }
        None
    }
}
