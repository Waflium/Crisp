use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct ProjectedRandomPatchySquare {
    block: BlockStateProviderHolder,
    project_through: BlockPredicate,
    size: IntProvider,
    max_projection_height: i32,
}

impl ProjectedRandomPatchySquare {
    pub fn parse(json: &Value) -> Result<ProjectedRandomPatchySquare, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(ProjectedRandomPatchySquare {
            block: BlockStateProviderHolder::parse(field("block")?)?,
            project_through: BlockPredicate::parse(field("project_through")?)?,
            size: IntProvider::parse(field("size")?)?,
            max_projection_height: field("max_projection_height")?.as_i64().ok_or("bad max_projection_height")? as i32,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, pos: BlockPos) -> bool {
        let size = self.size.sample(random);
        let bound = size * size + 1;
        for dx in -size..=size {
            for dz in -size..=size {
                let probability = dx.abs() * dz.abs();
                if random.next_int_bounded(bound) < bound - probability {
                    let mut base_pos = pos.offset(dx, 0, dz);
                    let mut drop = self.max_projection_height;
                    while self.project_through.test(level, base_pos.below()) {
                        base_pos = base_pos.below();
                        drop -= 1;
                        if drop <= 0 {
                            break;
                        }
                    }
                    if let Some(state) = self.block.value().get_optional_state(level, random, base_pos) {
                        level.set_block(base_pos, state, 2);
                    }
                }
            }
        }
        true
    }
}
