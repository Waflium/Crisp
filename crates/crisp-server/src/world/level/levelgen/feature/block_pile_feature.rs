use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::server::level::world_gen_region::WorldGenRegion;

pub struct BlockPileFeature {
    state_provider: BlockStateProviderHolder,
}

impl BlockPileFeature {
    pub fn parse(json: &Value) -> Result<BlockPileFeature, String> {
        Ok(BlockPileFeature { state_provider: BlockStateProviderHolder::parse(json.get("state_provider").ok_or("missing state_provider")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        if origin.y < level.get_min_y() + 5 {
            return false;
        }
        let xr = 2 + random.next_int_bounded(2);
        let zr = 2 + random.next_int_bounded(2);
        for z in origin.z - zr..=origin.z + zr {
            for y in origin.y..=origin.y + 1 {
                for x in origin.x - xr..=origin.x + xr {
                    let block_pos = BlockPos::new(x, y, z);
                    let xd = origin.x - block_pos.x;
                    let zd = origin.z - block_pos.z;
                    if ((xd * xd + zd * zd) as f32) <= random.next_float() * 10.0 - random.next_float() * 6.0 {
                        self.try_place_block(level, block_pos, random);
                    } else if (random.next_float() as f64) < 0.031 {
                        self.try_place_block(level, block_pos, random);
                    }
                }
            }
        }
        true
    }

    fn may_place_on(&self, level: &mut WorldGenRegion, block_pos: BlockPos, random: &mut impl RandomSource) -> bool {
        let below_state = level.get_block_state(block_pos.below());
        if below_state.is_block(blocks::DIRT_PATH) { random.next_boolean() } else { below_state.is_face_sturdy_full(Direction::Up) }
    }

    fn try_place_block<R: RandomSource>(&self, level: &mut WorldGenRegion, block_pos: BlockPos, random: &mut R) {
        if level.get_block_state(block_pos).is_air() && self.may_place_on(level, block_pos, random) {
            let state = self.state_provider.value().get_state(level, random, block_pos);
            level.set_block(block_pos, state, 260);
        }
    }
}
