use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::BlockState;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;
use crate::world::level::levelgen::parse_block_state;

pub struct BlockBlobFeature {
    state: BlockState,
    can_place_on: BlockPredicate,
}

impl BlockBlobFeature {
    pub fn parse(json: &Value) -> Result<BlockBlobFeature, String> {
        Ok(BlockBlobFeature {
            state: parse_block_state(json.get("state").ok_or("missing state")?)?,
            can_place_on: BlockPredicate::parse(json.get("can_place_on").ok_or("missing can_place_on")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, mut origin: BlockPos) -> bool {
        while origin.y > level.get_min_y() + 3 && !self.can_place_on.test(level, origin.below()) {
            origin = origin.below();
        }
        if origin.y <= level.get_min_y() + 3 {
            return false;
        }
        for _ in 0..3 {
            let xr = random.next_int_bounded(2);
            let yr = random.next_int_bounded(2);
            let zr = random.next_int_bounded(2);
            let tr = (xr + yr + zr) as f32 * 0.333 + 0.5;
            for z in origin.z - zr..=origin.z + zr {
                for y in origin.y - yr..=origin.y + yr {
                    for x in origin.x - xr..=origin.x + xr {
                        let block_pos = BlockPos::new(x, y, z);
                        if block_pos.dist_sqr(origin) <= (tr * tr) as f64 {
                            level.set_block_and_update(block_pos, self.state);
                        }
                    }
                }
            }
            let dx = -1 + random.next_int_bounded(2);
            let dy = -random.next_int_bounded(2);
            let dz = -1 + random.next_int_bounded(2);
            origin = origin.offset(dx, dy, dz);
        }
        true
    }
}
