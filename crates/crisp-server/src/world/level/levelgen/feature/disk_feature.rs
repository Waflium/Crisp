use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::feature::mark_above_for_post_processing;
use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;

pub struct DiskFeature {
    state_provider: BlockStateProviderHolder,
    target: BlockPredicate,
    radius: IntProvider,
    half_height: i32,
}

impl DiskFeature {
    pub fn parse(json: &Value) -> Result<DiskFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(DiskFeature {
            state_provider: BlockStateProviderHolder::parse(field("state_provider")?)?,
            target: BlockPredicate::parse(field("target")?)?,
            radius: IntProvider::parse(field("radius")?)?,
            half_height: field("half_height")?.as_i64().ok_or("bad half_height")? as i32,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut placed_any = false;
        let origin_y = origin.y;
        let top = origin_y + self.half_height;
        let bottom = origin_y - self.half_height - 1;
        let r = self.radius.sample(random);
        for z in origin.z - r..=origin.z + r {
            for x in origin.x - r..=origin.x + r {
                let xd = x - origin.x;
                let zd = z - origin.z;
                if xd * xd + zd * zd <= r * r {
                    placed_any |= self.place_column(level, random, top, bottom, x, z);
                }
            }
        }
        placed_any
    }

    fn place_column<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, top: i32, bottom: i32, x: i32, z: i32) -> bool {
        let mut placed_any = false;
        let mut placed_above = false;
        let mut y = top;
        while y > bottom {
            let pos = BlockPos::new(x, y, z);
            if self.target.test(level, pos) {
                if let Some(state) = self.state_provider.value().get_optional_state(level, random, pos) {
                    level.set_block(pos, state, 2);
                    if !placed_above {
                        mark_above_for_post_processing(level, pos);
                    }
                    placed_any = true;
                    placed_above = true;
                }
            } else {
                placed_above = false;
            }
            y -= 1;
        }
        placed_any
    }
}
