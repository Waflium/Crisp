use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::{AGE_1, BAMBOO_LEAVES, BambooLeaves, STAGE};
use crisp_data::{BlockState, block_tags, blocks};
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::float;
use crate::world::level::block;
use crate::world::level::chunk::HeightmapType;

pub struct BambooFeature {
    probability: f32,
}

fn bamboo_trunk() -> BlockState {
    blocks::BAMBOO.default_state().set(AGE_1, 1).set(BAMBOO_LEAVES, BambooLeaves::None).set(STAGE, 0)
}

impl BambooFeature {
    pub fn parse(json: &Value) -> Result<BambooFeature, String> {
        Ok(BambooFeature { probability: float(json.get("probability").ok_or("missing probability")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut placed = 0;
        let mut bamboo_pos = origin;
        if level.get_block_state(bamboo_pos).is_air() {
            let bamboo = blocks::BAMBOO.default_state();
            if block::of(bamboo).can_survive(bamboo, level, bamboo_pos) {
                let height = random.next_int_bounded(12) + 5;
                if random.next_float() < self.probability {
                    let r = random.next_int_bounded(4) + 1;
                    for xx in origin.x - r..=origin.x + r {
                        for zz in origin.z - r..=origin.z + r {
                            let xd = xx - origin.x;
                            let zd = zz - origin.z;
                            if xd * xd + zd * zd <= r * r {
                                let podzol_pos = BlockPos::new(xx, level.get_height(HeightmapType::WorldSurface, xx, zz) - 1, zz);
                                if level.get_block_state(podzol_pos).is(&block_tags::BENEATH_BAMBOO_PODZOL_REPLACEABLE) {
                                    level.set_block(podzol_pos, blocks::PODZOL.default_state(), 2);
                                }
                            }
                        }
                    }
                }
                let trunk = bamboo_trunk();
                let mut i = 0;
                while i < height && level.get_block_state(bamboo_pos).is_air() {
                    level.set_block(bamboo_pos, trunk, 2);
                    bamboo_pos = bamboo_pos.above();
                    i += 1;
                }
                if bamboo_pos.y - origin.y >= 3 {
                    level.set_block(bamboo_pos, trunk.set(BAMBOO_LEAVES, BambooLeaves::Large).set(STAGE, 1), 2);
                    bamboo_pos = bamboo_pos.below();
                    level.set_block(bamboo_pos, trunk.set(BAMBOO_LEAVES, BambooLeaves::Large), 2);
                    bamboo_pos = bamboo_pos.below();
                    level.set_block(bamboo_pos, trunk.set(BAMBOO_LEAVES, BambooLeaves::Small), 2);
                }
            }
            placed += 1;
        }
        placed > 0
    }
}
